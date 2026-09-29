#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! **DalgaKes (WaveClip)** — podcast kayıtlarından kural tabanlı *hook*
//! (ilginç an) adayı bulan ve altyazı üreten yerel komut satırı aracı.
//!
//! # Modüller
//!
//! - [`wav`] — RIFF/WAVE kapsayıcı ayrıştırma ve akış tabanlı PCM okuma.
//! - [`energy`] — sabit pencereli RMS enerji zarfı ve konuşma/sessizlik ayrımı.
//! - [`rules`] — kural tabanlı aday üretimi, örtüşen adayları birleştirme,
//!   tekrarları bastırma.
//! - [`subtitle`] — SRT ve WebVTT altyazı üretimi.
//! - [`rapor`] — JSON çıktı şeması.
//! - [`hata`] — hata sınıfları ve `std::error::Error` uygulaması.
//!
//! # Veri akışı
//!
//! ```text
//! WAV dosyası
//!   -> wav::WavOkuyucu        (akış; sabit boyutlu tampon)
//!   -> energy::EnerjiToplayici (sabit pencere, RMS serisi)
//!   -> rules::adaylar_uret     (kural adayları, birleştirme, tekrar bastırma)
//!   -> rapor::Rapor            (JSON)
//!   -> subtitle::{srt_yaz, vtt_yaz}
//! ```
//!
//! # Kapsam dışı (MANIFEST kart 02)
//!
//! Sıkıştırılmış ses (MP3/AAC/Opus), transkripsiyon, kelime düzeyinde
//! zamanlama, konuşmacı ayrımı ve altyazı basma bu sürümde yoktur. Gerekçesi
//! `MANIFEST.md` kart 02 "Sapma gerekçesi" bölümündedir.

pub mod energy;
pub mod hata;
pub mod rapor;
pub mod rules;
pub mod subtitle;
pub mod wav;

pub use energy::{EnerjiAyar, EnerjiToplayici, EnerjiZarfi};
pub use hata::{Hata, Sonuc};
pub use rapor::{KirpmaPlani, Klip, Rapor, SesOzeti};
pub use rules::{Aday, Kural, KuralAyar};
pub use subtitle::{AltyaziAyar, Ipucu};
pub use wav::{WavBaslik, WavOkuyucu};
