//! Segment başına RMS enerji zarfı ve konuşma/sessizlik ayrımı.
//!
//! Sorumluluk: akış hâlinde gelen mono örneklerden sabit pencere genişliğinde
//! (varsayılan 20 ms) RMS enerji serisi üretmek, uyarlanabilir taban gürültü
//! üzerinden konuşma/sessizlik kararı vermek.
//!
//! **Bellek sözleşmesi:** `EnerjiZarfi` yalnızca pencere başına bir `f64`
//! saklar. Kayıt ne kadar uzun olursa olsun *aktif* tampon (pencerenin içinde
//! biriken kısmi örnekler) sabittir. Test `buyuk_kayit_aktif_tamponu_degistirmez`
//! bunu doğrular.

use crate::hata::Sonuc;
use crate::wav::WavOkuyucu;

/// Varsayılan pencere genişliği (milisaniye).
pub const VARSAYILAN_PENCERE_MS: u32 = 20;

/// Akış okumasında kullanılan sabit örnek tamponu.
///
/// WAV okuyucusu bunu doldurup `EnerjiZarfi::ekle`ye geçirir; boyutu kayıt
/// uzunluğundan bağımsızdır.
pub const ORNEK_TAMPON: usize = 8192;

/// Enerji analizi ayarları.
#[derive(Debug, Clone, PartialEq)]
pub struct EnerjiAyar {
    /// Pencere genişliği (ms). `0` veya `1`'e düşürülürse 1'e sabitlenir.
    pub pencere_ms: u32,
    /// Taban gürültünün bu çarpan (dB cinsinden, `20*log10`) kadar üstü
    /// "konuşma" sayılır.
    pub esik_carpan_db: f64,
    /// Mutlak alt eşik (dBFS). Sayısallaştırma gürültüsünü "konuşma" saymaz.
    pub en_alcak_esik_db: f64,
    /// İki konuşma bölgesini birleştirmek için gereken en uzun aradaki düşüş
    /// süresi (ms). Kısa nefes araları bölgeyi bölmemesin.
    pub birlestirme_ara_ms: u32,
}

impl Default for EnerjiAyar {
    fn default() -> Self {
        EnerjiAyar {
            pencere_ms: VARSAYILAN_PENCERE_MS,
            esik_carpan_db: 10.0,
            en_alcak_esik_db: -60.0,
            birlestirme_ara_ms: 200,
        }
    }
}

/// Konuşma bölgesi: yalnız RMS değil, mantıklı bütünlük de taşır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KonusmaBolgesi {
    /// Başlangıç penceresi (dahil).
    pub bas: usize,
    /// Bitiş penceresi (hariç).
    pub bitis: usize,
}

impl KonusmaBolgesi {
    /// Bölgenin kare sayısı.
    pub fn uzunluk(&self) -> usize {
        self.bitis.saturating_sub(self.bas)
    }
}

/// Pencere bazlı enerji zarfı.
#[derive(Debug, Clone, PartialEq)]
pub struct EnerjiZarfi {
    /// Örnekleme hızı (Hz).
    pub ornekleme_hizi: u32,
    /// Bir pencerenin süresi (saniye).
    pub kare_sure_sn: f64,
    /// Pencere başına RMS (lineer, 0..1).
    pub rms: Vec<f64>,
    /// Pencere başına RMS'in dB karşılığı (tam sessizlikte `en_alcak_db`).
    pub rms_db: Vec<f64>,
    /// Konuşma kabul edilen alt eşik (dBFS).
    pub konusma_esigi_db: f64,
    /// Tahmin edilen taban gürültü (dBFS).
    pub taban_gurultu_db: f64,
    /// Bölge birleştirmede kullanılacak en uzun aradaki düşüş (kare sayısı).
    pub birlestirme_ara_kare: usize,
}

impl EnerjiZarfi {
    /// Zarfın toplam süresi (saniye).
    pub fn sure_sn(&self) -> f64 {
        self.kare_sure_sn * self.rms.len() as f64
    }

    /// Pencerenin başlangıç zamanı (saniye).
    pub fn kare_zamani(&self, kare: usize) -> f64 {
        kare as f64 * self.kare_sure_sn
    }

    /// En yüksek RMS (dBFS). Zarf boşsa tamamen dijital sessizlik değeri
    /// (`-120.0`) döner; JSON çıktısında `null`/`-Infinity` oluşmaz.
    pub fn tepe_db(&self) -> f64 {
        self.rms_db
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            .max(-120.0)
    }

    /// Ortalama RMS (dBFS). Zarf boşsa `en_alcak_db` döner.
    pub fn ortalama_db(&self) -> f64 {
        if self.rms_db.is_empty() {
            return self.konusma_esigi_db;
        }
        self.rms_db.iter().sum::<f64>() / self.rms_db.len() as f64
    }

    /// RMS'i dB'ye çevirir; sıfır için sonlu bir taban değer döner.
    pub fn rms_to_db(rms: f64) -> f64 {
        if rms <= 0.0 {
            return -120.0;
        }
        20.0 * rms.log10()
    }

    /// Sessizlik çözülmüş, en az `min_uzunluk_kare` uzunluğunda konuşma
    /// bölgelerini aralık sırasıyla döndürür.
    ///
    /// Aralıklar bitişik bölgeler birleştirilmiş hâlde elde edilir: iki
    /// konuşma arasındaki düşüş `birlestirme_ara_kare` pencereden kısaysa
    /// (nefes arası) bölge bölünmez, uzun sessizlikler böler.
    pub fn konusma_bolgeleri(&self, min_uzunluk_kare: usize) -> Vec<KonusmaBolgesi> {
        let esik = self.konusma_esigi_db;
        let n = self.rms_db.len();
        let mut bolgeler: Vec<KonusmaBolgesi> = Vec::new();
        let mut bas = 0usize;
        while bas < n {
            if self.rms_db[bas] < esik {
                bas += 1;
                continue;
            }
            let mut bitis = bas;
            while bitis < n {
                if self.rms_db[bitis] < esik {
                    // Kısa düşüşler (nefes araları) bölgeyi bölmemesi için
                    // `birlestirme_ara_kare` kadar ileriye bakılır.
                    let son = std::cmp::min(bitis + self.birlestirme_ara_kare, n);
                    let araliktaki_en_yuksek = self.rms_db[bitis..son]
                        .iter()
                        .copied()
                        .fold(f64::NEG_INFINITY, f64::max);
                    if araliktaki_en_yuksek >= esik {
                        bitis = son;
                        continue;
                    }
                    break;
                }
                bitis += 1;
            }
            if bitis > bas {
                bolgeler.push(KonusmaBolgesi { bas, bitis });
            }
            bas = std::cmp::max(bitis, bas + 1);
        }
        bolgeler.retain(|b| b.uzunluk() >= min_uzunluk_kare);
        bolgeler
    }

    /// Bir pencerenin sıfır tabanlı indeksi verilir; sınır dışıysa `0`.
    pub fn kare_indeksi(&self, zaman_sn: f64) -> usize {
        if self.kare_sure_sn <= 0.0 {
            return 0;
        }
        let i = (zaman_sn / self.kare_sure_sn).floor();
        if i < 0.0 {
            0
        } else {
            std::cmp::min(i as usize, self.rms.len())
        }
    }

    /// Sessizlik çözülmüş, verilen kare aralığındaki ortalama RMS.
    pub fn ortalama_rms(&self, bas: usize, bitis: usize) -> f64 {
        let b = std::cmp::min(bas, self.rms.len());
        let s = std::cmp::min(bitis, self.rms.len());
        if s <= b {
            return 0.0;
        }
        self.rms[b..s].iter().sum::<f64>() / (s - b) as f64
    }
}

/// Zarfı akış hâlinde üreten toplayıcı.
///
/// Tek geçişlidir: örnekler yazıldıkça pencere kareleri hesaplanır. İçinde
/// yalnızca kısmi pencere tutulur, bu yüzden `aktif_tampon_boyutu` sabittir.
pub struct EnerjiToplayici {
    ornekleme_hizi: u32,
    ayar: EnerjiAyar,
    kare_ornek: usize,
    kismi: Vec<f32>,
    kismi_toplam: f64,
    rms: Vec<f64>,
}

impl EnerjiToplayici {
    /// Yeni toplayıcı kurar.
    ///
    /// `ornekleme_hizi` 0 ise 1'e sabitlenir (bölme hatası yerine geçerli
    /// ama anlamsız bir zaman ekseni üretmektense çökmek yeğdir; bu yüzden
    /// 0 değeri düzeltilir).
    pub fn yeni(ornekleme_hizi: u32, ayar: EnerjiAyar) -> Self {
        let hiz = if ornekleme_hizi == 0 {
            1
        } else {
            ornekleme_hizi
        };
        let kare_ornek = std::cmp::max(
            1,
            (hiz as u64 * ayar.pencere_ms.max(1) as u64 / 1000) as usize,
        );
        EnerjiToplayici {
            ornekleme_hizi: hiz,
            ayar,
            kare_ornek,
            kismi: Vec::new(),
            kismi_toplam: 0.0,
            rms: Vec::new(),
        }
    }

    /// Bir penceredeki örnek sayısı.
    pub fn kare_ornek(&self) -> usize {
        self.kare_ornek
    }

    /// Kısmi pencerede bekleyen örnek sayısı — kayıt uzunluğundan bağımsız,
    /// en fazla `kare_ornek - 1`.
    pub fn aktif_tampon_boyutu(&self) -> usize {
        self.kismi.len()
    }

    /// Örnek dizisini ekler; tamamlanan her pencere için RMS hesaplanır.
    pub fn ekle(&mut self, ornekler: &[f32]) {
        for &o in ornekler {
            self.kismi.push(o);
            self.kismi_toplam += o as f64 * o as f64;
            if self.kismi.len() >= self.kare_ornek {
                self.kare_yaz();
            }
        }
    }

    fn kare_yaz(&mut self) {
        let n = self.kismi.len().max(1) as f64;
        let rms = (self.kismi_toplam / n).sqrt();
        self.rms.push(rms);
        self.kismi.clear();
        self.kismi_toplam = 0.0;
    }

    /// Akışı tamamlar ve zarfı üretir.
    ///
    /// Kalan kısmi pencere de tek bir kare olarak yazılır; böylece son kısa
    /// bölüm kaybolmaz.
    pub fn bitir(mut self) -> EnerjiZarfi {
        if !self.kismi.is_empty() {
            self.kare_yaz();
        }
        let kare_sure_sn = self.kare_ornek as f64 / self.ornekleme_hizi as f64;
        let rms_db: Vec<f64> = self
            .rms
            .iter()
            .map(|r| EnerjiZarfi::rms_to_db(*r))
            .collect();

        // Uyarlanabilir taban gürültü: RMS dağılımının alt yüzdeliği. Tamamen
        // sıfır olan ya da çok kısa zarlarda alt yüzdelik 0'a düşer; o durumda
        // tepe ile ortalama arasındaki fark korunur.
        let mut sirali = rms_db.clone();
        sirali.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let alt_yuzdelik = if sirali.is_empty() {
            self.ayar.en_alcak_esik_db
        } else {
            let i = ((sirali.len() as f64 * 0.10) as usize).min(sirali.len() - 1);
            sirali[i]
        };
        let tepe_db = rms_db.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let ortalama_db = if rms_db.is_empty() {
            self.ayar.en_alcak_esik_db
        } else {
            rms_db.iter().sum::<f64>() / rms_db.len() as f64
        };
        let taban = if tepe_db <= -119.0 {
            // Tamamen dijital sessizlik: tepe eşiğin altındadır.
            self.ayar.en_alcak_esik_db
        } else {
            alt_yuzdelik.max(ortalama_db - 40.0).max(-100.0)
        };
        let esik = (taban + self.ayar.esik_carpan_db).max(self.ayar.en_alcak_esik_db);
        // ms -> kare: (ms / 1000) / kare_sure_sn. Örnekleme hızı sadeleşir,
        // çünkü hem süre hem kare uzunluğu hıza orantılıdır.
        let birlestirme_ara_kare = std::cmp::max(
            1,
            ((self.ayar.birlestirme_ara_ms as f64 / 1000.0) / kare_sure_sn).round() as usize,
        );

        EnerjiZarfi {
            ornekleme_hizi: self.ornekleme_hizi,
            kare_sure_sn,
            rms: self.rms,
            rms_db,
            konusma_esigi_db: esik,
            taban_gurultu_db: taban,
            birlestirme_ara_kare,
        }
    }

    /// Sıfır uzunluklu zarf üretir (okuyucu hiç örnek vermediyse).
    pub fn bos_zarf(ornekleme_hizi: u32, ayar: &EnerjiAyar) -> EnerjiZarfi {
        EnerjiToplayici::yeni(ornekleme_hizi, ayar.clone()).bitir()
    }
}

/// Bir WAV dosyasının tamamını akış hâlinde okuyup enerji zarfı üretir.
///
/// Ses verisi belleğe alınmaz: sabit boyutlu `ORNEK_TAMPON` tamponu kullanılır
/// ve pencere sonuçları (`Vec<f64>`) kayıt süresiyle orantılıdır — bu, tek
/// doğrusal büyümedir ve 60 dakikalık kayıtta ~180 000 kare × 8 bayt ≈ 1.4 MB
/// demektir.
pub fn zarf_uret(okuyucu: &mut WavOkuyucu, ayar: &EnerjiAyar) -> Sonuc<EnerjiZarfi> {
    let ornekleme_hizi = okuyucu.baslik().ornekleme_hizi;
    let mut toplayici = EnerjiToplayici::yeni(ornekleme_hizi, ayar.clone());
    let mut tampon = vec![0f32; ORNEK_TAMPON];
    loop {
        let n = okuyucu.sonraki_blok(&mut tampon)?;
        if n == 0 {
            break;
        }
        toplayici.ekle(&tampon[..n]);
    }
    Ok(toplayici.bitir())
}

impl std::io::Read for EnerjiToplayici {
    /// Enerji toplayıcı bir `Wav` okuyucusu değildir; bu `Read` uygulaması
    /// yalnızca `std::io::Read` sınırını karşılamak içindir ve her zaman
    /// "bitti" döner. Ses verisi bu yapıdan akış olarak **okunmaz**.
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ayar(pencere_ms: u32) -> EnerjiAyar {
        EnerjiAyar {
            pencere_ms,
            ..EnerjiAyar::default()
        }
    }

    fn zarfi_uret(hiz: u32, ay: &EnerjiAyar, veri: &[f32]) -> EnerjiZarfi {
        let mut t = EnerjiToplayici::yeni(hiz, ay.clone());
        t.ekle(veri);
        t.bitir()
    }

    /// Tam genlikli sinüs üretir (deterministik, `sin` tabanlı).
    fn sinus(frekans: f64, hiz: u32, sure_sn: f64) -> Vec<f32> {
        let n = (hiz as f64 * sure_sn) as usize;
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * frekans * i as f64 / hiz as f64).sin() as f32)
            .collect()
    }

    #[test]
    fn yirmi_ms_pencerede_kare_sayisi_dogru() {
        let veri = vec![0.5f32; 8000]; // 1 sn @ 8 kHz -> 50 kare
        let z = zarfi_uret(8000, &ayar(20), &veri);
        assert_eq!(z.rms.len(), 50);
        assert!((z.kare_sure_sn - 0.02).abs() < 1e-12);
        assert!((z.sure_sn() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sabit_sinyal_rms_degerine_yakin() {
        let veri = vec![0.5f32; 8000];
        let z = zarfi_uret(8000, &ayar(20), &veri);
        let ilk = z.rms[0];
        assert!((ilk - 0.5).abs() < 1e-6, "rms {ilk}");
    }

    #[test]
    fn sinus_rms_beklenen_degere_yakin() {
        // 1 kHz tam genlik sinüs RMS = 1/sqrt(2) ~= 0.7071
        let veri = sinus(1000.0, 8000, 1.0);
        let z = zarfi_uret(8000, &ayar(20), &veri);
        for r in &z.rms {
            assert!(
                (r - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.01,
                "rms {r}"
            );
        }
    }

    #[test]
    fn duz_dc_ofset_rms_i_otomatik_gunceller() {
        // Düz DC ofseti enerji olarak sayılır: RMS = ofset. Bilinen sınırlama
        // README "Bilinen Sınırlamalar" bölümünde belgelidir.
        let veri = vec![0.8f32; 8000];
        let z = zarfi_uret(8000, &ayar(20), &veri);
        assert!((z.rms[0] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn dc_ofsetli_sinuste_konusma_esigi_yukselir() {
        // Sinüs + 0.5 DC: RMS ≈ sqrt(0.5 + 0.25) ≈ 0.866, sadece AC'nin
        // 0.707'si değil. Test bu bilinen davranışı sabitler.
        let mut veri = sinus(1000.0, 8000, 1.0);
        for v in veri.iter_mut() {
            *v += 0.5;
        }
        let z = zarfi_uret(8000, &ayar(20), &veri);
        let beklenen = (0.5f64 + 0.25f64).sqrt();
        assert!((z.rms[0] - beklenen).abs() < 0.01, "rms {}", z.rms[0]);
        // Ofset yükseldiği için taban gürültü de yükselir.
        let z_sade = zarfi_uret(8000, &ayar(20), &sinus(1000.0, 8000, 1.0));
        assert!(z.taban_gurultu_db > z_sade.taban_gurultu_db);
    }

    #[test]
    fn sessiz_dosya_tum_kareler_sifir() {
        let veri = vec![0f32; 8000];
        let z = zarfi_uret(8000, &ayar(20), &veri);
        assert_eq!(z.rms.len(), 50);
        assert!(z.rms.iter().all(|r| *r == 0.0));
        assert_eq!(z.tepe_db(), -120.0);
        assert_eq!(z.taban_gurultu_db, -60.0);
        assert!(z.konusma_bolgeleri(1).is_empty());
    }

    #[test]
    fn bos_girdi_bos_zarf_uretir() {
        let z = zarfi_uret(8000, &ayar(20), &[]);
        assert!(z.rms.is_empty());
        assert_eq!(z.sure_sn(), 0.0);
        // Boş zarfta eşik, mutlak alt sınırın üstündeki karşılıktır.
        assert_eq!(z.ortalama_db(), z.konusma_esigi_db);
        assert_eq!(z.tepe_db(), -120.0);
    }

    #[test]
    fn kismi_pencere_son_kareye_yazilir() {
        // 10 örnek, 20 ms pencere (8000 Hz'de 160 örnek): hiç tam kare olmaz.
        let veri = vec![1.0f32; 10];
        let z = zarfi_uret(8000, &ayar(20), &veri);
        assert_eq!(z.rms.len(), 1, "kismi pencere de yazilmali");
    }

    #[test]
    fn aktif_tampon_sabit_kalir() {
        let mut t = EnerjiToplayici::yeni(8000, ayar(20));
        let kare = t.kare_ornek();
        for _ in 0..1000 {
            t.ekle(&[0.25f32; 10]);
            assert!(t.aktif_tampon_boyutu() < kare);
        }
        let z = t.bitir();
        assert_eq!(z.rms.len(), 1000 * 10 / kare + 1);
    }

    #[test]
    fn rms_to_db_sifirda_sonlu_doner() {
        assert_eq!(EnerjiZarfi::rms_to_db(0.0), -120.0);
        assert!((EnerjiZarfi::rms_to_db(1.0)).abs() < 1e-12);
        assert!((EnerjiZarfi::rms_to_db(0.5) - 20.0 * 0.5f64.log10()).abs() < 1e-12);
    }

    #[test]
    fn konusma_bolgeleri_sessizlikle_bolunur() {
        // 0.5 sn sessiz, 1 sn konuşma (0.8 genlik), 0.5 sn sessiz
        let mut veri = vec![0.0f32; 4000];
        veri.extend(vec![0.8f32; 8000]);
        veri.extend(vec![0.0f32; 4000]);
        let z = zarfi_uret(8000, &ayar(20), &veri);
        let bolgeler = z.konusma_bolgeleri(5);
        assert_eq!(bolgeler.len(), 1, "{bolgeler:?}");
        let b = bolgeler[0];
        // Konuşma gerçekte 0.5-1.5 sn aralığında; eşik kenar kareleri
        // tam olarak bu sınırları verir (genlik sabit olduğu için taşma yok).
        assert!(
            (z.kare_zamani(b.bas) - 0.5).abs() < 0.03,
            "{}",
            z.kare_zamani(b.bas)
        );
        assert!(
            (z.kare_zamani(b.bitis) - 1.5).abs() < 0.03,
            "{}",
            z.kare_zamani(b.bitis)
        );
    }

    #[test]
    fn kisa_ses_icinde_konusma_bolgesi_yok() {
        // 0.04 sn konuşma: 2 kareden az, eşik altı.
        let mut veri = vec![0.0f32; 400];
        veri.extend(vec![0.9f32; 300]);
        veri.extend(vec![0.0f32; 400]);
        let z = zarfi_uret(8000, &ayar(20), &veri);
        assert!(z.konusma_bolgeleri(5).is_empty());
    }

    #[test]
    fn kare_indeksi_sinirlari_kirpar() {
        let veri = vec![0.5f32; 8000];
        let z = zarfi_uret(8000, &ayar(20), &veri);
        assert_eq!(z.kare_indeksi(-5.0), 0);
        assert_eq!(z.kare_indeksi(0.0), 0);
        assert_eq!(z.kare_indeksi(0.21), 10);
        assert_eq!(z.kare_indeksi(999.0), 50);
    }

    #[test]
    fn ortalama_rms_aranin_disinda_sifir() {
        let veri = vec![0.5f32; 8000];
        let z = zarfi_uret(8000, &ayar(20), &veri);
        assert!((z.ortalama_rms(0, 10) - 0.5).abs() < 1e-6);
        assert_eq!(z.ortalama_rms(40, 10), 0.0);
        assert_eq!(z.ortalama_rms(100, 200), 0.0);
    }

    #[test]
    fn taban_gurultu_uyarlanabilir() {
        // Düşük gürültülü zemin + yüksek konuşma: taban zeminin seviyesinde.
        let mut veri = vec![0.001f32; 8000];
        veri.extend(vec![0.6f32; 8000]);
        let z = zarfi_uret(8000, &ayar(20), &veri);
        assert!(
            z.taban_gurultu_db < -40.0,
            "taban {} cok yuksek",
            z.taban_gurultu_db
        );
        assert!(z.konusma_esigi_db > z.taban_gurultu_db);
    }

    #[test]
    fn sifir_ornekleme_hizi_duzeltilir() {
        let mut t = EnerjiToplayici::yeni(0, ayar(20));
        // 1 Hz'de 20 ms penceresi tam bir örnekten kısa; en az 1 örnek alınır.
        assert_eq!(t.kare_ornek(), 1);
        t.ekle(&[0.5, 0.5]);
        let z = t.bitir();
        assert_eq!(z.ornekleme_hizi, 1);
        assert_eq!(z.rms.len(), 2);
    }

    #[test]
    fn bos_zarf_yardimcisi_calisir() {
        let z = EnerjiToplayici::bos_zarf(44100, &EnerjiAyar::default());
        assert!(z.rms.is_empty());
        assert_eq!(z.ornekleme_hizi, 44100);
    }
}
