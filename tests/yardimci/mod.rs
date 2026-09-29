//! Entegrasyon testleri için ortak yardımcılar: geçici dizin ve WAV üretici.
//!
//! `tempfile` bağımlılığı yasaktır (WORKER_CONTRACT.md § 3.2-F); geçici dizin
//! `std::env::temp_dir()` altında, `std::process::id()` ile benzersizleştirilerek
//! üretilir.

use std::path::PathBuf;

/// Test içinde geçici dosya/dizin üreten, `Drop` ile temizleyen kapsayıcı.
///
/// Neden `tempfile` yok: bağımlılık politikası (WORKER_CONTRACT § 3.2) `tempfile`'i
/// hiçbir projede vermez; yardımcı kendi kodumuzla yazılır.
pub struct GeciciDizin {
    yol: PathBuf,
}

impl GeciciDizin {
    /// `std::env::temp_dir()` altında, etiketten türetilmiş benzersiz dizin üretir.
    ///
    /// Etiket testler arasında **benzersiz olmalıdır**: testler aynı süreçte
    /// iş parçacıkları olarak paralel çalıştığı için aynı etiket aynı yolu
    /// üretir ve iki test birbirinin dosyalarını temizler.
    pub fn yeni(etiket: &str) -> std::io::Result<Self> {
        let kok = std::env::temp_dir().join(format!("waveclip-{etiket}-{}", std::process::id()));
        // Aynı testin iki kez çalışması olası; eski içerik önce silinir.
        let _ = std::fs::remove_dir_all(&kok);
        std::fs::create_dir_all(&kok)?;
        Ok(Self { yol: kok })
    }

    /// Dizine göreli dosya yolu üretir.
    pub fn dosya(&self, ad: &str) -> PathBuf {
        self.yol.join(ad)
    }
}

impl Drop for GeciciDizin {
    fn drop(&mut self) {
        // Temizlik başarısız olsa da testi düşürmemeli; `let _ =` bilinçlidir.
        // `Drop` içinden hata döndürülemez.
        let _ = std::fs::remove_dir_all(&self.yol);
    }
}

/// Ses üretim tarifi.
#[derive(Debug, Clone)]
pub struct Tarif {
    /// Örnekleme hızı (Hz).
    pub hiz: u32,
    /// Kanal sayısı.
    pub kanal: u16,
    /// Kanal başına bit genişliği (8/16/24/32).
    pub bit: u16,
    /// Sırayla eklenecek bölgeler: `(süre_saniye, genlik)`.
    pub bolgeler: Vec<(f64, f32)>,
    /// Genliğe eklenecek DC ofseti.
    pub dc_ofset: f32,
    /// Sinyal frekansı (Hz).
    pub frekans: f64,
}

impl Tarif {
    /// Kısa, 16-bit mono, üç bölgeli podcast benzeri kayıt.
    pub fn ornek() -> Tarif {
        Tarif {
            hiz: 8000,
            kanal: 1,
            bit: 16,
            bolgeler: vec![(2.0, 0.0), (1.0, 0.7), (3.0, 0.0), (1.0, 0.95), (3.0, 0.0)],
            dc_ofset: 0.0,
            frekans: 220.0,
        }
    }
}

/// Tariften ham PCM bayt dizisi üretir.
pub fn pcm_uret(tarif: &Tarif) -> Vec<u8> {
    let bayt = (tarif.bit / 8) as usize;
    let toplam: usize = tarif
        .bolgeler
        .iter()
        .map(|(s, _)| (s * tarif.hiz as f64) as usize)
        .sum();
    let mut ornekler: Vec<f32> = Vec::with_capacity(toplam);
    let mut faze = 0.0f64;
    for (sure, genlik) in &tarif.bolgeler {
        let n = (*sure * tarif.hiz as f64) as usize;
        for _ in 0..n {
            let deger = if *genlik == 0.0 {
                0.0
            } else {
                let s =
                    (2.0 * std::f64::consts::PI * tarif.frekans * faze / tarif.hiz as f64).sin();
                faze += 1.0;
                (*genlik as f64 * s) as f32
            };
            ornekler.push(deger + tarif.dc_ofset);
        }
    }
    let mut cikti = Vec::with_capacity(ornekler.len() * bayt * tarif.kanal as usize);
    for o in &ornekler {
        for _ in 0..tarif.kanal {
            // `ornek_yaz` sabit 4 baytlık tampon döndürür; yalnız `bayt` kadarı
            // kopyalanır (8-bit'te 1, 16-bit'te 2, 24-bit'te 3, 32-bit'te 4).
            let tampon = ornek_yaz(*o, tarif.bit);
            cikti.extend_from_slice(&tampon[..bayt]);
        }
    }
    cikti
}

/// Örneği 4 baytlık sabit tampona yazar; tüketici yalnız `bit/8` bayt alır.
fn ornek_yaz(deger: f32, bit: u16) -> [u8; 4] {
    let v = deger.clamp(-1.0, 1.0);
    match bit {
        8 => [((v * 127.0).round() as i16 + 128) as u8, 0, 0, 0],
        16 => {
            let b = ((v * 32767.0).round() as i16).to_le_bytes();
            [b[0], b[1], 0, 0]
        }
        24 => {
            let i = (v * 8_388_607.0).round() as i32;
            [
                (i & 0xFF) as u8,
                ((i >> 8) & 0xFF) as u8,
                ((i >> 16) & 0xFF) as u8,
                0,
            ]
        }
        32 => ((v * 2_147_483_647.0).round() as i32).to_le_bytes(),
        _ => [0, 0, 0, 0],
    }
}

/// `LIST`/`INFO` alanı gövdesi üretir.
pub fn info_alani(id: &str, deger: &str) -> Vec<u8> {
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

/// RIFF/WAVE dosyası üretir.
pub fn wav_ustur(tarif: &Tarif, veri: &[u8], info: Option<&[u8]>) -> Vec<u8> {
    let blok = (tarif.bit / 8) * tarif.kanal;
    let mut govde: Vec<u8> = Vec::new();
    govde.extend_from_slice(b"WAVE");

    govde.extend_from_slice(b"fmt ");
    govde.extend_from_slice(&16u32.to_le_bytes());
    govde.extend_from_slice(&1u16.to_le_bytes());
    govde.extend_from_slice(&tarif.kanal.to_le_bytes());
    govde.extend_from_slice(&tarif.hiz.to_le_bytes());
    govde.extend_from_slice(&(tarif.hiz * blok as u32).to_le_bytes());
    govde.extend_from_slice(&blok.to_le_bytes());
    govde.extend_from_slice(&tarif.bit.to_le_bytes());

    if let Some(alanlar) = info {
        // LIST kutusu: kimlik, boyut, ardından "INFO" liste türü ve alt kutular.
        govde.extend_from_slice(b"LIST");
        govde.extend_from_slice(&((alanlar.len() + 4) as u32).to_le_bytes());
        govde.extend_from_slice(b"INFO");
        govde.extend_from_slice(alanlar);
    }

    govde.extend_from_slice(b"data");
    govde.extend_from_slice(&(veri.len() as u32).to_le_bytes());
    govde.extend_from_slice(veri);
    if veri.len() % 2 == 1 {
        govde.push(0);
    }

    let mut cikti: Vec<u8> = Vec::new();
    cikti.extend_from_slice(b"RIFF");
    cikti.extend_from_slice(&(govde.len() as u32).to_le_bytes());
    cikti.extend_from_slice(&govde);
    cikti
}

/// Tariften doğrudan WAV dosyası yazar ve yolunu döndürür.
pub fn wav_yaz(
    dizin: &GeciciDizin,
    ad: &str,
    tarif: &Tarif,
) -> std::io::Result<std::path::PathBuf> {
    let veri = pcm_uret(tarif);
    let mut info = Vec::new();
    info.extend_from_slice(&info_alani("INAM", "Bolum 12"));
    info.extend_from_slice(&info_alani("IART", "Test Konukmaci"));
    let baytlar = wav_ustur(tarif, &veri, Some(&info));
    let yol = dizin.dosya(ad);
    std::fs::write(&yol, baytlar)?;
    Ok(yol)
}
