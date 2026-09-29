//! Hata tipi ve `std::error::Error` uygulaması.
//!
//! Sorumluluk yalnızca "hataları tek bir enum altında toplamak ve okunabilir
//! biçimde yazdırmaktır". Hata sınıfları ayrı tutulur çünkü "ses çözülemez",
//! "kapsayıcı bozuk" ve "WAV değil" durumlarının çözümü farklıdır (rapor b07).
//!
//! `thiserror` bağımlılığı yasaktır (WORKER_CONTRACT.md § 3.2-F), bu yüzden
//! `Display` elle yazılmıştır.

use std::fmt;
use std::io;

/// WaveClip'in tüm hata sınıfları.
///
/// Kullanıcı girdisinden kaynaklanan her hata bu enum üzerinden döner;
/// üretim kodunda `panic!` çağrısı yapılmaz.
#[derive(Debug)]
#[non_exhaustive]
pub enum Hata {
    /// Dosya sistemi hatası (açma, okuma, yazma).
    Io(io::Error),
    /// Dosya RIFF/WAVE kapsayıcısı değil (ör. MP3, FLAC, MP4).
    KapsayiciDegil {
        /// Okunan ilk dört baytın ASCII karşılığı.
        imza: String,
    },
    /// RIFF başlığı var ama gövdesi 12 bayttan kısa.
    CabukBaslik {
        /// Bulunan bayt sayısı.
        bulunan: usize,
    },
    /// `fmt ` kutusu yok veya 16 bayttan kısa.
    FormKutusuYok,
    /// `data` kutusu yok.
    VeriKutusuYok,
    /// Kutu başlığı kırpılmış (dosya beklenenden erken bitiyor).
    BozukKutu {
        /// Kutu kimliği, örneğin `"data"`.
        kimlik: String,
        /// Bildirilen uzunluk.
        bildirilen: u64,
        /// Dosyada kalan bayt.
        kalan: u64,
    },
    /// Desteklenmeyen ses biçimi (yalnızca PCM: 8/16/24/32 bit).
    DesteklenmeyenBicim {
        /// `wFormatTag` değeri.
        bicim_kodu: u16,
    },
    /// Sıfır kanal veya sıfır örnekleme hızı gibi anlamsız başlık değerleri.
    GecersizBaslik {
        /// Sorunun açıklaması.
        ayrinti: String,
    },
    /// JSON okuma/yazma hatası.
    Json(String),
    /// Komut satırı doğrulama hatası.
    Ayarlar(String),
}

impl fmt::Display for Hata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hata::Io(h) => write!(f, "dosya sistemi hatasi: {h}"),
            Hata::KapsayiciDegil { imza } => write!(
                f,
                "dosya RIFF/WAVE degil (imza: {imza:?}); yalnizca PCM iceren WAV okunur, \
                 MP3/AAC/Opus desteklenmez"
            ),
            Hata::CabukBaslik { bulunan } => write!(
                f,
                "RIFF basligi cabuk: beklenen en az 12 bayt, dosyada {bulunan} bayt var"
            ),
            Hata::FormKutusuYok => write!(f, "'fmt ' kutusu yok veya 16 bayttan kisa"),
            Hata::VeriKutusuYok => write!(f, "'data' kutusu yok"),
            Hata::BozukKutu {
                kimlik,
                bildirilen,
                kalan,
            } => write!(
                f,
                "'{kimlik}' kutusu kirpilmis: {bildirilen} bayt bildirilmis, dosyada {kalan} bayt var"
            ),
            Hata::DesteklenmeyenBicim { bicim_kodu } => write!(
                f,
                "desteklenmeyen ses bicimi (wFormatTag = {bicim_kodu}); yalnizca PCM \
                 (1) 8/16/24/32 bit desteklenir"
            ),
            Hata::GecersizBaslik { ayrinti } => write!(f, "gecersiz ses basligi: {ayrinti}"),
            Hata::Json(m) => write!(f, "JSON hatasi: {m}"),
            Hata::Ayarlar(m) => write!(f, "ayar hatasi: {m}"),
        }
    }
}

impl std::error::Error for Hata {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Hata::Io(h) => Some(h),
            _ => None,
        }
    }
}

impl From<io::Error> for Hata {
    fn from(deger: io::Error) -> Self {
        Hata::Io(deger)
    }
}

/// Sonucu `Hata` olan işlemlerin kısa sonuç tipi.
pub type Sonuc<T> = Result<T, Hata>;
