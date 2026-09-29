//! JSON çıktı şeması: kayıt bilgisi, enerji özeti ve aday listesi.
//!
//! Sorumluluk: `scan` çıktısının serileştirilmesi ve `srt`/`info` komutlarının
//! okuyabileceği biçimde geri okunması.
//!
//! Şema kararları MANIFEST kart 02 madde 6'dan gelir: `kaynak`, `sure_sn`,
//! `aday_sayisi`, `kural_surumu` alanları zorunludur.

use serde::{Deserialize, Serialize};

use crate::energy::EnerjiZarfi;
use crate::hata::{Hata, Sonuc};
use crate::rules::{Aday, Kural, KuralAyar};

/// Kural setinin sürüm damgası.
///
/// Şema veya kural davranışı değiştiğinde artırılır; rapor b07 "kural seti
/// sürüm damgalıdır" kararını böyle karşılar.
pub const KURAL_SURESI: &str = "waveclip-kural-1.0.0";

/// Enerji analizi özeti.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnerjiOzeti {
    /// Pencere genişliği (ms).
    pub pencere_ms: u32,
    /// Toplam kare sayısı.
    pub kare_sayisi: usize,
    /// Tahmin edilen taban gürültü (dBFS).
    pub taban_gurultu_db: f64,
    /// Konuşma alt eşiği (dBFS).
    pub konusma_esigi_db: f64,
    /// Ortalama RMS (dBFS).
    pub ortalama_db: f64,
    /// En yüksek RMS (dBFS).
    pub tepe_db: f64,
}

impl EnerjiOzeti {
    /// Zarftan özet üretir.
    pub fn zarfdan(zarf: &EnerjiZarfi, pencere_ms: u32) -> Self {
        EnerjiOzeti {
            pencere_ms,
            kare_sayisi: zarf.rms.len(),
            taban_gurultu_db: zarf.taban_gurultu_db,
            konusma_esigi_db: zarf.konusma_esigi_db,
            ortalama_db: zarf.ortalama_db(),
            tepe_db: zarf.tepe_db(),
        }
    }
}

/// `scan` komutunun tam çıktısı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rapor {
    /// Kaynak dosya adı.
    pub kaynak: String,
    /// Kayıt süresi (saniye).
    pub sure_sn: f64,
    /// Aday sayısı.
    pub aday_sayisi: usize,
    /// Kural seti sürüm damgası.
    pub kural_surumu: String,
    /// Ses başlığı özeti.
    pub ses: SesOzeti,
    /// Enerji analizi özeti.
    pub enerji: EnerjiOzeti,
    /// Aday listesi (güven puanına göre sıralı).
    pub adaylar: Vec<Aday>,
}

impl Rapor {
    /// Özet bilgilerden rapor kurar.
    pub fn olustur(
        kaynak: String,
        ses: SesOzeti,
        zarf: &EnerjiZarfi,
        pencere_ms: u32,
        adaylar: Vec<Aday>,
        ayar: &KuralAyar,
    ) -> Self {
        let _ = ayar;
        Rapor {
            kaynak,
            sure_sn: zarf.sure_sn(),
            aday_sayisi: adaylar.len(),
            kural_surumu: KURAL_SURESI.to_string(),
            ses,
            enerji: EnerjiOzeti::zarfdan(zarf, pencere_ms),
            adaylar,
        }
    }

    /// Raporu okunabilir JSON metnine çevirir.
    pub fn metin(&self) -> Sonuc<String> {
        serde_json::to_string_pretty(self).map_err(|h| Hata::Json(h.to_string()))
    }

    /// JSON metninden rapor okur.
    pub fn metinden(metin: &str) -> Sonuc<Self> {
        serde_json::from_str(metin).map_err(|h| Hata::Json(h.to_string()))
    }
}

/// Ses başlığının rapordaki hâli.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SesOzeti {
    /// `wFormatTag`.
    pub bicim_kodu: u16,
    /// Kanal sayısı.
    pub kanal_sayisi: u16,
    /// Örnekleme hızı (Hz).
    pub ornekleme_hizi: u32,
    /// Kanal başına bit genişliği.
    pub bit_derinligi: u16,
    /// Kısa insan okunur özet.
    pub ozet: String,
    /// `LIST`/`INFO` alanları.
    pub meta: Vec<(String, String)>,
}

impl SesOzeti {
    /// `WavBaslik`'ten rapor özetine çevirir.
    pub fn basliktan(baslik: &crate::wav::WavBaslik) -> Self {
        SesOzeti {
            bicim_kodu: baslik.bicim_kodu,
            kanal_sayisi: baslik.kanal_sayisi,
            ornekleme_hizi: baslik.ornekleme_hizi,
            bit_derinligi: baslik.bit_derinligi,
            ozet: baslik.ozet(),
            meta: baslik
                .meta
                .iter()
                .map(|m| (m.kimlik.clone(), m.deger.clone()))
                .collect(),
        }
    }
}

/// `clips` komutunun çıktısı: onaylanmış adayların kırpma planı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KirpmaPlani {
    /// Kaynak dosya adı.
    pub kaynak: String,
    /// Kural seti sürüm damgası.
    pub kural_surumu: String,
    /// Uygulanan zaman kaydırması (saniye).
    pub kaydirma_sn: f64,
    /// Kayıt süresi (saniye).
    pub sure_sn: f64,
    /// Kırpılacak klip sayısı.
    pub klip_sayisi: usize,
    /// Klipler (zaman sırasıyla).
    pub klipler: Vec<Klip>,
}

/// Tek bir klip kaydı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Klip {
    /// Klip başlangıcı (saniye, kaydırma uygulanmış).
    pub baslangic_sn: f64,
    /// Klip bitişi (saniye, kaydırma uygulanmış).
    pub bitis_sn: f64,
    /// Kaynak adayın güven puanı.
    pub guven: f64,
    /// Kaynak adayı hangi kurallarla üretildi.
    pub kurallar: Vec<Kural>,
    /// Kırpma gerekçesi (kullanıcıya gösterilir).
    pub gerekce: String,
}

impl KirpmaPlani {
    /// Rapor + onaylı aday sıralarından kırpma planı üretir.
    ///
    /// `kaydirma_sn` kadar kaydırılır ve aralıklar `[0, sure_sn]` içine
    /// kırpılır; sıfır uzunlukta kalan klipler elenir.
    pub fn olustur(kaynak: &str, sure_sn: f64, kaydirma_sn: f64, adaylar: &[Aday]) -> Self {
        let mut klipler: Vec<Klip> = Vec::new();
        for aday in adaylar {
            let bas = (aday.baslangic_sn + kaydirma_sn).clamp(0.0, sure_sn);
            let bit = (aday.bitis_sn + kaydirma_sn).clamp(0.0, sure_sn);
            if bit - bas <= 0.0 {
                continue;
            }
            klipler.push(Klip {
                baslangic_sn: bas,
                bitis_sn: bit,
                guven: aday.guven,
                kurallar: aday.kurallar.clone(),
                gerekce: aday.gerekce.clone(),
            });
        }
        klipler.sort_by(|a, b| {
            a.baslangic_sn
                .partial_cmp(&b.baslangic_sn)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        KirpmaPlani {
            kaynak: kaynak.to_string(),
            kural_surumu: KURAL_SURESI.to_string(),
            kaydirma_sn,
            sure_sn,
            klip_sayisi: klipler.len(),
            klipler,
        }
    }

    /// Planı okunabilir JSON metnine çevirir.
    pub fn metin(&self) -> Sonuc<String> {
        serde_json::to_string_pretty(self).map_err(|h| Hata::Json(h.to_string()))
    }

    /// JSON metninden plan okur.
    pub fn metinden(metin: &str) -> Sonuc<Self> {
        serde_json::from_str(metin).map_err(|h| Hata::Json(h.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ornek_aday() -> Aday {
        Aday {
            baslangic_sn: 5.0,
            bitis_sn: 25.0,
            guven: 0.8,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "test".to_string(),
        }
    }

    #[test]
    fn rapor_json_uzerinden_gidis_donus() {
        let mut t =
            crate::energy::EnerjiToplayici::yeni(8000, crate::energy::EnerjiAyar::default());
        t.ekle(&vec![0.4f32; 1600]);
        let z = t.bitir();
        let r = Rapor::olustur(
            "a.wav".to_string(),
            SesOzeti {
                bicim_kodu: 1,
                kanal_sayisi: 1,
                ornekleme_hizi: 8000,
                bit_derinligi: 16,
                ozet: "test".to_string(),
                meta: Vec::new(),
            },
            &z,
            20,
            vec![ornek_aday()],
            &KuralAyar::default(),
        );
        let metin = r.metin().expect("serilestirilmeli");
        let geri = Rapor::metinden(&metin).expect("cozumlenmeli");
        assert_eq!(r, geri);
        assert_eq!(geri.kural_surumu, KURAL_SURESI);
        assert_eq!(geri.aday_sayisi, 1);
    }

    #[test]
    fn bozuk_json_hata_verir() {
        let hata = Rapor::metinden("{ bozuk").expect_err("hata beklenir");
        assert!(matches!(hata, Hata::Json(_)));
    }

    #[test]
    fn kirpma_plani_kaydirma_uygular() {
        let plan = KirpmaPlani::olustur("a.wav", 100.0, 3.0, &[ornek_aday()]);
        assert_eq!(plan.klip_sayisi, 1);
        assert!((plan.klipler[0].baslangic_sn - 8.0).abs() < 1e-9);
        assert!((plan.klipler[0].bitis_sn - 28.0).abs() < 1e-9);
    }

    #[test]
    fn kirpma_plani_kayit_disina_kirpar() {
        let plan = KirpmaPlani::olustur("a.wav", 10.0, 100.0, &[ornek_aday()]);
        assert_eq!(plan.klip_sayisi, 0);
    }

    #[test]
    fn kirpma_plani_sifir_sureli_klipi_eler() {
        // Tam kayıt uzunluğunda bir aday, 0 kaydırmayla yine de > 0 kalır;
        // ancak kaydırma tam süre kadar negatif başlangıç verir.
        let aday = Aday {
            baslangic_sn: 0.0,
            bitis_sn: 10.0,
            guven: 0.5,
            kurallar: vec![Kural::IlkAcilis],
            gerekce: "t".to_string(),
        };
        let plan = KirpmaPlani::olustur("a.wav", 5.0, -10.0, &[aday]);
        assert_eq!(plan.klip_sayisi, 0);
    }

    #[test]
    fn kirpma_plani_json_gidis_donus() {
        let plan = KirpmaPlani::olustur("a.wav", 100.0, 0.0, &[ornek_aday()]);
        let metin = plan.metin().expect("serilestirilmeli");
        let geri = KirpmaPlani::metinden(&metin).expect("cozumlenmeli");
        assert_eq!(plan, geri);
    }

    #[test]
    fn kirpma_plani_zaman_sirasina_gorer_siralanir() {
        let gec = Aday {
            baslangic_sn: 50.0,
            bitis_sn: 60.0,
            guven: 0.5,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "g".to_string(),
        };
        let plan = KirpmaPlani::olustur("a.wav", 100.0, 0.0, &[gec, ornek_aday()]);
        assert!(plan.klipler[0].baslangic_sn < plan.klipler[1].baslangic_sn);
    }

    #[test]
    fn enerji_ozeti_zarftan_dogurulur() {
        let mut t =
            crate::energy::EnerjiToplayici::yeni(8000, crate::energy::EnerjiAyar::default());
        t.ekle(&vec![0.5f32; 800]);
        let z = t.bitir();
        let o = EnerjiOzeti::zarfdan(&z, 20);
        assert_eq!(o.pencere_ms, 20);
        assert_eq!(o.kare_sayisi, z.rms.len());
        assert!(o.ortalama_db > -120.0);
    }
}
