//! RIFF/WAVE kapsayıcı ayrıştırıcı ve akış tabanlı PCM okuyucu.
//!
//! Sorumluluk: kapsayıcıyı (`RIFF`/`WAVE`, `fmt `, `data`, `LIST`/`INFO`),
//! PCM örneklerini 8/16/24/32 bit genişlikte okumak ve **akış hâlinde** vermek.
//!
//! Sorumluluk dışı: sıkıştırılmış biçimler (MP3, AAC, Opus, ADPCM) bilinçli
//! olarak reddedilir — MANIFEST kart 02 bunları MVP dışı bırakmıştır.
//!
//! **Bellek sözleşmesi:** ses verisi hiçbir zaman tümüyle belleğe alınmaz.
//! `WavOkuyucu` yalnızca 8 baytlık kutu başlıklarını ve `data` kutusundan
//! çağıranın verdiği sabit boyutlu bloğu okur. `data` kutusu 4 GiB'a kadar
//! olabilir; okuyucunun ayrılmış belleği bununla orantılı değildir.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::hata::{Hata, Sonuc};

/// `data` kutusunun akış okumasında kullanılacak varsayılan blok bayt sayısı.
///
/// Bu değer `WavOkuyucu` içinde **sabit** bir tampon olarak yaşar; kayıt
/// uzunluğu arttıkça büyümez. Test `buyuk_dosyada_tampon_sabit_kalir` bunu
/// doğrular.
pub const OKUMA_BLOK_BAYT: usize = 64 * 1024;

/// `LIST`/`INFO` kutusundan okunan tek bir metadata alanı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaAlani {
    /// Dört karakterlik alt küme kimliği, örneğin `INAM`.
    pub kimlik: String,
    /// Kimliğe karşılık gelen metin (sonundaki NUL ve boşluklar temizlenmiş).
    pub deger: String,
}

/// Ses başlığının çözülmüş hâli.
#[derive(Debug, Clone, PartialEq)]
pub struct WavBaslik {
    /// `wFormatTag` (1 = PCM, 3 = IEEE float — float reddedilir).
    pub bicim_kodu: u16,
    /// Kanal sayısı (1 = mono, 2 = stereo).
    pub kanal_sayisi: u16,
    /// Örnekleme hızı (ör. 44100).
    pub ornekleme_hizi: u32,
    /// Kanal başına örnek genişliği (8/16/24/32).
    pub bit_derinligi: u16,
    /// Kutu bildirimine göre bayt/sn (`nAvgBytesPerSec`).
    pub bayt_hizi: u32,
    /// Bir karedeki (tüm kanallar) bayt sayısı (`nBlockAlign`).
    pub blok_hizasi: u16,
    /// `data` kutusunun dosya içindeki başlangıç ofseti.
    pub veri_ofseti: u64,
    /// `data` kutusunun bildirilen uzunluğu; dosyada kalanla sınırlanır.
    pub veri_boyutu: u64,
    /// `LIST`/`INFO` alanları (yoksa boş vektör).
    pub meta: Vec<MetaAlani>,
}

impl WavBaslik {
    /// Kısa biçimlendirme, ör. `"PCM 44100 Hz, 2 kanal, 16 bit"`.
    pub fn ozet(&self) -> String {
        format!(
            "PCM {} Hz, {} kanal, {} bit",
            self.ornekleme_hizi, self.kanal_sayisi, self.bit_derinligi
        )
    }

    /// Kırpılmış dosyada `data` kutusu dosya sonundan taşabilir; bu durumda
    /// kullanılabilir gerçek bayt sayısını döndürür.
    pub fn kullanilabilir_veri_boyutu(&self) -> u64 {
        self.veri_boyutu
    }

    /// Kayıt süresi (saniye). Kırpılmış dosyada mevcut veriden hesaplanır.
    pub fn sure_sn(&self) -> f64 {
        let kare_bayt = self.blok_hizasi as f64;
        if kare_bayt <= 0.0 {
            return 0.0;
        }
        self.kullanilabilir_veri_boyutu() as f64 / kare_bayt / self.ornekleme_hizi as f64
    }

    /// Mono kare sayısı (kanal sayısından bağımsız zaman ekseni).
    pub fn kare_sayisi(&self) -> u64 {
        if self.blok_hizasi == 0 {
            return 0;
        }
        self.kullanilabilir_veri_boyutu() / self.blok_hizasi as u64
    }
}

/// Kutu başlığı: dört bayt kimlik + dört bayt (little-endian) uzunluk.
struct KutuBasligi {
    kimlik: [u8; 4],
    boyut: u32,
}

fn kutu_basligi_oku<R: Read>(okuyucu: &mut R) -> Sonuc<Option<KutuBasligi>> {
    let mut ham = [0u8; 8];
    let mut alinan = 0usize;
    while alinan < 8 {
        match okuyucu.read(&mut ham[alinan..]) {
            Ok(0) => break,
            Ok(n) => alinan += n,
            Err(h) if h.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(h) => return Err(Hata::Io(h)),
        }
    }
    if alinan == 0 {
        return Ok(None);
    }
    if alinan < 8 {
        return Err(Hata::BozukKutu {
            kimlik: "(kutu basligi)".to_string(),
            bildirilen: 8,
            kalan: alinan as u64,
        });
    }
    let boyut = u32::from_le_bytes([ham[4], ham[5], ham[6], ham[7]]);
    Ok(Some(KutuBasligi {
        kimlik: [ham[0], ham[1], ham[2], ham[3]],
        boyut,
    }))
}

fn le_u16(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn ascii_kimlik(k: &[u8; 4]) -> String {
    k.iter().map(|&b| b as char).collect()
}

/// `LIST`/`INFO` gövdesini `(kimlik, metin)` çiftlerine ayırır.
fn info_govdesi_coz(govde: &[u8]) -> Vec<MetaAlani> {
    let mut alanlar = Vec::new();
    let mut i = 0usize;
    while i + 8 <= govde.len() {
        let kimlik = ascii_kimlik(&[govde[i], govde[i + 1], govde[i + 2], govde[i + 3]]);
        let boyut = le_u32(&govde[i + 4..i + 8]) as usize;
        i += 8;
        let son = std::cmp::min(i + boyut, govde.len());
        if son <= i {
            // Sıfır uzunluklu ya da dosya sonunu aşan alt kutu: burada durulur,
            // aksi halde döngü ilerlemez.
            break;
        }
        let ham = &govde[i..son];
        let metin = String::from_utf8_lossy(ham)
            .trim_matches(['\0', ' ', '\r', '\n'])
            .to_string();
        if !metin.is_empty() {
            alanlar.push(MetaAlani {
                kimlik,
                deger: metin,
            });
        }
        i = son + (boyut & 1); // RIFF hizalama dolgusu
    }
    alanlar
}

/// Yalnızca başlığı okur; ses verisine dokunmaz.
///
/// `data` kutusu dosya sonundan taşıyorsa `BozukKutu` yerine **kırpılmış
/// dosya** kabul edilir ve `veri_boyutu` mevcut bayt sayısıyla sınırlanır.
/// Bu, "kırpılmış dosya" kenar durumunun çökme yerine yararlı sonuç vermesini
/// sağlar.
pub fn baslik_oku(yol: &Path) -> Sonuc<WavBaslik> {
    let dosya = File::open(yol)?;
    let mut okuyucu = BufReader::with_capacity(OKUMA_BLOK_BAYT, dosya);
    baslik_oku_akistan(&mut okuyucu)
}

/// `baslik_oku` işlemini bir akış üzerinde yapar (testler ve alt akışlar için).
pub fn baslik_oku_akistan<R: Read + Seek>(okuyucu: &mut R) -> Sonuc<WavBaslik> {
    let mut baslik = [0u8; 12];
    let mut alinan = 0usize;
    while alinan < 12 {
        match okuyucu.read(&mut baslik[alinan..]) {
            Ok(0) => break,
            Ok(n) => alinan += n,
            Err(h) if h.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(h) => return Err(Hata::Io(h)),
        }
    }
    if alinan >= 4 && &baslik[0..4] != b"RIFF" {
        return Err(Hata::KapsayiciDegil {
            imza: ascii_kimlik(&[baslik[0], baslik[1], baslik[2], baslik[3]]),
        });
    }
    if alinan < 12 {
        return Err(Hata::CabukBaslik { bulunan: alinan });
    }
    if &baslik[8..12] != b"WAVE" {
        return Err(Hata::KapsayiciDegil {
            imza: ascii_kimlik(&[baslik[8], baslik[9], baslik[10], baslik[11]]),
        });
    }

    let riff_boyut = le_u32(&baslik[4..8]) as u64;
    let dosya_boyutu = okuyucu.seek(SeekFrom::End(0))?;
    // RIFF boyutu genellikle 4 bayt eksiktir (WAVE imzası dahil değildir);
    // güvenli taraf için dosya uzunluğu esas alınır.
    let _ = riff_boyut;
    okuyucu.seek(SeekFrom::Start(12))?;

    let mut bicim_kodu = 0u16;
    let mut kanal_sayisi = 0u16;
    let mut ornekleme_hizi = 0u32;
    let mut bayt_hizi = 0u32;
    let mut blok_hizasi = 0u16;
    let mut bit_derinligi = 0u16;
    let mut format_var = false;

    let mut veri_ofseti: Option<u64> = None;
    let mut veri_boyutu: u64 = 0;
    let mut meta: Vec<MetaAlani> = Vec::new();

    let mut ofset = 12u64;
    while ofset + 8 <= dosya_boyutu {
        okuyucu.seek(SeekFrom::Start(ofset))?;
        let bazlik = match kutu_basligi_oku(okuyucu)? {
            Some(k) => k,
            None => break,
        };
        let govde_ofset = ofset + 8;
        let bildirilen = bazlik.boyut as u64;
        let gercek = std::cmp::min(bildirilen, dosya_boyutu.saturating_sub(govde_ofset));

        match bazlik.kimlik.as_slice() {
            b"fmt " => {
                if gercek < 16 {
                    return Err(Hata::FormKutusuYok);
                }
                let mut g = [0u8; 16];
                okuyucu.read_exact(&mut g)?;
                bicim_kodu = le_u16(&g[0..2]);
                kanal_sayisi = le_u16(&g[2..4]);
                ornekleme_hizi = le_u32(&g[4..8]);
                bayt_hizi = le_u32(&g[8..12]);
                blok_hizasi = le_u16(&g[12..14]);
                bit_derinligi = le_u16(&g[14..16]);
                format_var = true;
            }
            b"data" => {
                veri_ofseti = Some(govde_ofset);
                veri_boyutu = gercek;
            }
            b"LIST" if gercek >= 4 => {
                let mut tur = [0u8; 4];
                okuyucu.read_exact(&mut tur)?;
                if &tur == b"INFO" {
                    let mut govde = vec![0u8; (gercek - 4) as usize];
                    okuyucu.read_exact(&mut govde)?;
                    meta.extend(info_govdesi_coz(&govde));
                }
            }
            _ => {}
        }

        // `data` kutusunu bulduktan sonra daha fazla kutu okumaya gerek yok;
        // erken çıkış uzun dosyalarda başlık ayrıştırmasını hızlandırır.
        if veri_ofseti.is_some() && format_var {
            break;
        }

        ofset = govde_ofset + bildirilen + (bildirilen & 1); // RIFF hizalama
    }

    if !format_var {
        return Err(Hata::FormKutusuYok);
    }
    let veri_ofseti = match veri_ofseti {
        Some(o) => o,
        None => return Err(Hata::VeriKutusuYok),
    };

    if bicim_kodu != 1 {
        return Err(Hata::DesteklenmeyenBicim { bicim_kodu });
    }
    if !matches!(bit_derinligi, 8 | 16 | 24 | 32) {
        return Err(Hata::DesteklenmeyenBicim { bicim_kodu: 0 });
    }
    if kanal_sayisi == 0 || ornekleme_hizi == 0 {
        return Err(Hata::GecersizBaslik {
            ayrinti: format!("kanal_sayisi={kanal_sayisi}, ornekleme_hizi={ornekleme_hizi}"),
        });
    }
    let beklenen_blok = (bit_derinligi / 8) * kanal_sayisi;
    if blok_hizasi == 0 {
        blok_hizasi = beklenen_blok;
    }
    if blok_hizasi < beklenen_blok {
        return Err(Hata::GecersizBaslik {
            ayrinti: format!("nBlockAlign={blok_hizasi} < kanal*bit/8={beklenen_blok}"),
        });
    }
    if bayt_hizi == 0 {
        bayt_hizi = ornekleme_hizi * blok_hizasi as u32;
    }

    Ok(WavBaslik {
        bicim_kodu,
        kanal_sayisi,
        ornekleme_hizi,
        bit_derinligi,
        bayt_hizi,
        blok_hizasi,
        veri_ofseti,
        veri_boyutu,
        meta,
    })
}

/// Akış tabanlı PCM okuyucu.
///
/// Örnekler mono `f32` olarak normalize edilmiş `[-1.0, 1.0]` aralığında
/// verilir; stereo kanallar ortalama alınarak mono'ya indirgenir.
pub struct WavOkuyucu {
    okuyucu: BufReader<File>,
    baslik: WavBaslik,
    kalan: u64,
    tampon: Vec<u8>,
    /// Bir önceki okumadan artan, tamamlanmamış kareler (stereo 24-bit gibi
    /// blok hizasını tam doldurmayan durumlarda).
    artik: Vec<u8>,
    uretilen_ornek: u64,
    toplam_ornek: u64,
}

impl WavOkuyucu {
    /// Verilen WAV yolunu açar ve akışı başlangıç konumuna getirir.
    pub fn ac(yol: &Path) -> Sonuc<Self> {
        let dosya = File::open(yol)?;
        let mut okuyucu = BufReader::with_capacity(OKUMA_BLOK_BAYT, dosya);
        let baslik = baslik_oku_akistan(&mut okuyucu)?;
        okuyucu.seek(SeekFrom::Start(baslik.veri_ofseti))?;
        let toplam_ornek = baslik.kare_sayisi() * baslik.kanal_sayisi as u64;
        let kalan = baslik.veri_boyutu;
        let artik_kap = baslik.blok_hizasi as usize;
        Ok(WavOkuyucu {
            okuyucu,
            baslik,
            kalan,
            tampon: vec![0u8; OKUMA_BLOK_BAYT],
            // Blok hizası tamamlanmamış kareler için ayrılan alan. Kapasite
            // baştan bir kareye (blok_hizasi) sabitlenir, böylece
            // `tampon_boyutu` kayıt uzunluğundan bağımsız olarak sabittir.
            artik: Vec::with_capacity(artik_kap),
            uretilen_ornek: 0,
            toplam_ornek,
        })
    }

    /// Ses başlığının kopyasını döndürür.
    pub fn baslik(&self) -> &WavBaslik {
        &self.baslik
    }

    /// Okuyucunun ayırdığı sabit boyutlu ham tamponun bayt sayısı.
    ///
    /// Test, kayıt süresi büyüdükçe bu değerin değişmediğini doğrular.
    pub fn tampon_boyutu(&self) -> usize {
        self.tampon.len() + self.artik.capacity()
    }

    /// Üretilmiş toplam örnek sayısı (kanal çözülmeden önceki kare sayısı × kanal).
    pub fn uretilen_ornek(&self) -> u64 {
        self.uretilen_ornek
    }

    /// Kayıttaki toplam örnek sayısı.
    pub fn toplam_ornek(&self) -> u64 {
        self.toplam_ornek
    }

    /// Sıradaki mono örnekleri `cikti` tamponuna yazar, kaç tane yazdığını döndürür.
    ///
    /// Dönüş `0` ise akış bitti demektir.
    pub fn sonraki_blok(&mut self, cikti: &mut [f32]) -> Sonuc<usize> {
        let blok_bayt = self.baslik.blok_hizasi as usize;
        if blok_bayt == 0 || self.kalan == 0 {
            return Ok(0);
        }
        let kare_sayisi = cikti.len();
        let mut yaz = 0usize;

        while yaz < kare_sayisi {
            while self.artik.len() < blok_bayt {
                let gereken = std::cmp::min(blok_bayt - self.artik.len(), self.kalan as usize);
                if gereken == 0 {
                    break;
                }
                let bas = self.tampon.len() - gereken;
                self.tampon[bas..bas + gereken].fill(0);
                let okunan = self.okuyucu.read(&mut self.tampon[bas..bas + gereken])?;
                if okunan == 0 {
                    // Dosya kırpılmış veya okuma sonuna gelindi: kalan bayt yok.
                    self.kalan = 0;
                    break;
                }
                self.artik
                    .extend_from_slice(&self.tampon[bas..bas + okunan]);
                self.kalan -= okunan as u64;
            }
            // Kare tamamlanamadıysa (kırpılmış dosya) akış biter.
            if self.artik.len() < blok_bayt {
                break;
            }
            let kare = &self.artik[..blok_bayt];
            cikti[yaz] = kare_ortalamasi(kare, &self.baslik);
            yaz += 1;
            self.uretilen_ornek += self.baslik.kanal_sayisi as u64;
            self.artik.drain(..blok_bayt);
        }
        Ok(yaz)
    }
}

/// Bir karedeki tüm kanalları normalize edip ortalar; sonucu `f32` döndürür.
fn kare_ortalamasi(kare: &[u8], baslik: &WavBaslik) -> f32 {
    let bayt = (baslik.bit_derinligi / 8) as usize;
    let kanal = baslik.kanal_sayisi as usize;
    if bayt == 0 || kanal == 0 {
        return 0.0;
    }
    let mut toplam = 0.0f64;
    for k in 0..kanal {
        let bas = k * bayt;
        toplam += ornek_normalize(&kare[bas..bas + bayt], baslik.bit_derinligi);
    }
    (toplam / kanal as f64) as f32
}

/// Ham kare baytlarını `[-1.0, 1.0]` aralığına normalize eder.
///
/// 8-bit PCM WAV'te **imzasızdır** (128 = 0); 16/24/32-bit imzalıdır.
pub fn ornek_normalize(bayt: &[u8], bit_derinligi: u16) -> f64 {
    match bit_derinligi {
        8 => {
            if bayt.is_empty() {
                return 0.0;
            }
            (bayt[0] as f64 - 128.0) / 128.0
        }
        16 => {
            if bayt.len() < 2 {
                return 0.0;
            }
            le_u16(bayt) as i16 as f64 / 32768.0
        }
        24 => {
            if bayt.len() < 3 {
                return 0.0;
            }
            let ham = (bayt[0] as i32) | ((bayt[1] as i32) << 8) | ((bayt[2] as i32) << 16);
            // 24-bit en üst bayt işaret uzantısı taşır; 32-bit'e doğru genişlet.
            let genis = if ham & 0x0080_0000 != 0 {
                ham | !0x00FF_FFFF
            } else {
                ham
            };
            genis as f64 / 8_388_608.0
        }
        32 => {
            if bayt.len() < 4 {
                return 0.0;
            }
            le_u32(bayt) as i32 as f64 / 2_147_483_648.0
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Test WAV üreticisi: RIFF/WAVE + `fmt ` + isteğe bağlı `LIST/INFO` + `data`.
    pub(crate) fn wav_burst_olustur(
        bicim_kodu: u16,
        kanal: u16,
        ornekleme_hizi: u32,
        bit: u16,
        veri: &[u8],
    ) -> Vec<u8> {
        let blok = (bit / 8) * kanal;
        let mut govde: Vec<u8> = Vec::new();
        govde.extend_from_slice(b"WAVE");
        govde.extend_from_slice(b"fmt ");
        govde.extend_from_slice(&(16u32).to_le_bytes());
        govde.extend_from_slice(&bicim_kodu.to_le_bytes());
        govde.extend_from_slice(&kanal.to_le_bytes());
        govde.extend_from_slice(&ornekleme_hizi.to_le_bytes());
        govde.extend_from_slice(&(ornekleme_hizi * blok as u32).to_le_bytes());
        govde.extend_from_slice(&blok.to_le_bytes());
        govde.extend_from_slice(&bit.to_le_bytes());
        govde.extend_from_slice(b"data");
        govde.extend_from_slice(&(veri.len() as u32).to_le_bytes());
        govde.extend_from_slice(veri);

        let mut cikti: Vec<u8> = Vec::new();
        cikti.extend_from_slice(b"RIFF");
        cikti.extend_from_slice(&(govde.len() as u32).to_le_bytes());
        cikti.extend_from_slice(&govde);
        cikti
    }

    pub(crate) fn info_alan(id: &str, deger: &str) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(id.as_bytes());
        let mut govde = deger.as_bytes().to_vec();
        if govde.len() % 2 == 1 {
            govde.push(0);
        }
        v.extend_from_slice(&(govde.len() as u32).to_le_bytes());
        v.extend_from_slice(&govde);
        v
    }

    fn uygula(bayt: &[u8]) -> Sonuc<WavBaslik> {
        baslik_oku_akistan(&mut Cursor::new(bayt.to_vec()))
    }

    #[test]
    fn baslik_oku_16bit_mono_44100_basar() {
        let veri = vec![0u8; 200]; // 100 kare @ 16 bit
        let w = wav_burst_olustur(1, 1, 44100, 16, &veri);
        let b = uygula(&w).expect("baslik okunmali");
        assert_eq!(b.kanal_sayisi, 1);
        assert_eq!(b.ornekleme_hizi, 44100);
        assert_eq!(b.bit_derinligi, 16);
        assert_eq!(b.kare_sayisi(), 100);
        assert!((b.sure_sn() - 100.0 / 44100.0).abs() < 1e-9);
    }

    #[test]
    fn riff_imzasi_olmayan_dosya_reddedilir() {
        let hata = uygula(b"ID3\x04\x00\x00\x00\x00\x00\x00").expect_err("hata beklenir");
        assert!(matches!(hata, Hata::KapsayiciDegil { .. }));
    }

    #[test]
    fn riff_ama_wave_imzasi_yok_reddedilir() {
        let mut w = b"RIFF".to_vec();
        w.extend_from_slice(&20u32.to_le_bytes());
        w.extend_from_slice(b"AVI ");
        w.extend_from_slice(&[0u8; 16]);
        let hata = uygula(&w).expect_err("hata beklenir");
        assert!(matches!(hata, Hata::KapsayiciDegil { .. }));
    }

    #[test]
    fn fmt_kutusu_eksikse_hata_verir() {
        let mut govde = b"WAVEfmt ".to_vec();
        govde.extend_from_slice(&8u32.to_le_bytes());
        govde.extend_from_slice(&[0u8; 8]);
        govde.extend_from_slice(b"data");
        govde.extend_from_slice(&0u32.to_le_bytes());
        let mut w = b"RIFF".to_vec();
        w.extend_from_slice(&(govde.len() as u32).to_le_bytes());
        w.extend_from_slice(&govde);
        let hata = uygula(&w).expect_err("hata beklenir");
        assert!(matches!(hata, Hata::FormKutusuYok));
    }

    #[test]
    fn data_kutusu_yoksa_hata_verir() {
        let mut govde = b"WAVEfmt ".to_vec();
        govde.extend_from_slice(&16u32.to_le_bytes());
        govde.extend_from_slice(&1u16.to_le_bytes());
        govde.extend_from_slice(&1u16.to_le_bytes());
        govde.extend_from_slice(&8000u32.to_le_bytes());
        govde.extend_from_slice(&16000u32.to_le_bytes());
        govde.extend_from_slice(&2u16.to_le_bytes());
        govde.extend_from_slice(&16u16.to_le_bytes());
        govde.extend_from_slice(b"LIST");
        govde.extend_from_slice(&4u32.to_le_bytes());
        govde.extend_from_slice(b"INFO");
        let mut w = b"RIFF".to_vec();
        w.extend_from_slice(&(govde.len() as u32).to_le_bytes());
        w.extend_from_slice(&govde);
        let hata = uygula(&w).expect_err("hata beklenir");
        assert!(matches!(hata, Hata::VeriKutusuYok));
    }

    #[test]
    fn kirpilmis_data_kutusu_kismi_olarak_kabul_edilir() {
        let veri = vec![1u8, 2, 3, 4, 5, 6];
        let mut w = wav_burst_olustur(1, 1, 8000, 16, &veri);
        // data boyutunu 1000 yap ama dosyayı kırp: gövde 6 bayt kalsın.
        // `data` kimliği dosyanın son 14 baytında, boyut alanı ondan sonraki 4.
        let boyut_ofseti = w.len() - 6;
        w[boyut_ofseti..boyut_ofseti + 4].copy_from_slice(&1000u32.to_le_bytes());
        let b = uygula(&w).expect("kirpma toleransi");
        assert_eq!(b.veri_boyutu, 6);
        assert_eq!(b.kare_sayisi(), 3);
    }

    #[test]
    fn sikistirilmis_bicim_reddedilir() {
        let w = wav_burst_olustur(0x11, 1, 8000, 4, &[0u8; 64]);
        let hata = uygula(&w).expect_err("hata beklenir");
        assert!(matches!(
            hata,
            Hata::DesteklenmeyenBicim { bicim_kodu: 0x11 }
        ));
    }

    #[test]
    fn bit_derinligi_dort_bit_reddedilir() {
        // 4-bit PCM WAV'ta standart değildir; kanal*bit/8 = 0 olurdu.
        let w = wav_burst_olustur(1, 1, 8000, 4, &[0u8; 64]);
        let hata = uygula(&w).expect_err("hata beklenir");
        assert!(matches!(hata, Hata::DesteklenmeyenBicim { .. }));
    }

    #[test]
    fn kanal_sayisi_sifir_reddedilir() {
        let w = wav_burst_olustur(1, 0, 8000, 16, &[0u8; 64]);
        let hata = uygula(&w).expect_err("hata beklenir");
        assert!(matches!(hata, Hata::GecersizBaslik { .. }));
    }

    #[test]
    fn list_info_metadatasi_okunur() {
        let mut alanlar = b"INFO".to_vec();
        alanlar.extend_from_slice(&info_alan("INAM", "Bolum 12"));
        alanlar.extend_from_slice(&info_alan("IART", "Tekni"));
        let mut govde = b"WAVEfmt ".to_vec();
        govde.extend_from_slice(&16u32.to_le_bytes());
        govde.extend_from_slice(&1u16.to_le_bytes());
        govde.extend_from_slice(&1u16.to_le_bytes());
        govde.extend_from_slice(&8000u32.to_le_bytes());
        govde.extend_from_slice(&16000u32.to_le_bytes());
        govde.extend_from_slice(&2u16.to_le_bytes());
        govde.extend_from_slice(&16u16.to_le_bytes());
        govde.extend_from_slice(b"LIST");
        govde.extend_from_slice(&(alanlar.len() as u32).to_le_bytes());
        govde.extend_from_slice(&alanlar);
        govde.extend_from_slice(b"data");
        govde.extend_from_slice(&32u32.to_le_bytes());
        govde.extend_from_slice(&[0u8; 32]);
        let mut w = b"RIFF".to_vec();
        w.extend_from_slice(&(govde.len() as u32).to_le_bytes());
        w.extend_from_slice(&govde);
        let b = uygula(&w).expect("baslik okunmali");
        assert_eq!(b.meta.len(), 2);
        assert_eq!(b.meta[0].kimlik, "INAM");
        assert_eq!(b.meta[0].deger, "Bolum 12");
        assert_eq!(b.meta[1].deger, "Tekni");
    }

    #[test]
    fn bilinmeyen_kutular_atlanir() {
        let mut govde = b"WAVEJUNK".to_vec();
        govde.extend_from_slice(&4u32.to_le_bytes());
        govde.extend_from_slice(&[9, 9, 9, 9]);
        govde.extend_from_slice(b"fmt ");
        govde.extend_from_slice(&16u32.to_le_bytes());
        govde.extend_from_slice(&1u16.to_le_bytes());
        govde.extend_from_slice(&1u16.to_le_bytes());
        govde.extend_from_slice(&8000u32.to_le_bytes());
        govde.extend_from_slice(&16000u32.to_le_bytes());
        govde.extend_from_slice(&2u16.to_le_bytes());
        govde.extend_from_slice(&16u16.to_le_bytes());
        govde.extend_from_slice(b"data");
        govde.extend_from_slice(&4u32.to_le_bytes());
        govde.extend_from_slice(&[0u8; 4]);
        let mut w = b"RIFF".to_vec();
        w.extend_from_slice(&(govde.len() as u32).to_le_bytes());
        w.extend_from_slice(&govde);
        let b = uygula(&w).expect("baslik okunmali");
        assert_eq!(b.kare_sayisi(), 2);
    }

    #[test]
    fn normalize_8bit_imzasiz_dogru() {
        assert!((ornek_normalize(&[128], 8) - 0.0).abs() < 1e-12);
        assert!((ornek_normalize(&[255], 8) - 127.0 / 128.0).abs() < 1e-12);
        assert!((ornek_normalize(&[0], 8) + 1.0).abs() < 1e-12);
    }

    #[test]
    fn normalize_16_24_32_bit_dogru() {
        // 0x8000 = -32768 olarak okunur -> -1.0
        assert!((ornek_normalize(&(-32768i16).to_le_bytes(), 16) + 1.0).abs() < 1e-12);
        assert!((ornek_normalize(&(32767i16).to_le_bytes(), 16) - 32767.0 / 32768.0).abs() < 1e-12);
        assert!((ornek_normalize(&(-32768i16).to_le_bytes(), 16) + 1.0).abs() < 1e-12);
        // 24-bit: 0x800000 -> -1.0
        let m24 = [0x00u8, 0x00, 0x80];
        assert!((ornek_normalize(&m24, 24) + 1.0).abs() < 1e-12);
        let p24 = [0x00u8, 0x00, 0x40];
        assert!((ornek_normalize(&p24, 24) - 0.5).abs() < 1e-12);
        let m32 = i32::MIN.to_le_bytes();
        assert!((ornek_normalize(&m32, 32) + 1.0).abs() < 1e-12);
    }

    #[test]
    fn kisa_tampon_normalize_sifir_dondurur() {
        assert_eq!(ornek_normalize(&[], 16), 0.0);
        assert_eq!(ornek_normalize(&[1], 16), 0.0);
        assert_eq!(ornek_normalize(&[1, 2], 24), 0.0);
        assert_eq!(ornek_normalize(&[1, 2, 3], 32), 0.0);
        assert_eq!(ornek_normalize(&[1], 12), 0.0);
    }

    #[test]
    fn bos_veri_sifir_kare_verir() {
        let w = wav_burst_olustur(1, 1, 8000, 16, &[]);
        let b = uygula(&w).expect("baslik okunmali");
        assert_eq!(b.kare_sayisi(), 0);
        assert_eq!(b.sure_sn(), 0.0);
    }
}
