#![forbid(unsafe_code)]

//! WaveClip komut satırı arayüzü.
//!
//! Alt komutlar: `scan` (adayları bul), `clips` (kırpma planı), `srt`
//! (altyazı üret), `info` (başlık özeti). Çekirdek mantık `waveclip::`
//! kütüphanesindedir; burada yalnızca argüman ayrıştırma ve dosya yazımı vardır.

use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use waveclip::energy::{zarf_uret, EnerjiAyar};
use waveclip::hata::Sonuc;
use waveclip::rapor::{KirpmaPlani, Rapor, SesOzeti};
use waveclip::rules::{self, KuralAyar};
use waveclip::subtitle::{self, AltyaziAyar};
use waveclip::wav::{baslik_oku, WavOkuyucu};

/// DalgaKes — podcast kayıtlarından hook adayı bulan yerel araç.
#[derive(Debug, Parser)]
#[command(
    name = "waveclip",
    version,
    about = "Podcast WAV kaydindan kural tabanli hook (ilginc an) adayi bulur ve SRT/VTT altyazi uretir.",
    long_about = "DalgaKes (WaveClip): PCM iceren RIFF/WAVE dosyalarini akis halinde okur, \
20 ms pencereli RMS enerji zarfini uretir, dort kural tabanli hook adayi cikarir ve \
onaylanan araliklardan SRT veya WebVTT altyazi yazar. Sikistirilmis ses desteklenmez."
)]
struct Cli {
    #[command(subcommand)]
    komut: Komut,
}

#[derive(Debug, Subcommand)]
enum Komut {
    /// Bir WAV dosyasını tarayıp hook adaylarını JSON olarak raporlar.
    Scan(ScanArg),
    /// Onaylanan adaylardan kırpma planı (JSON) üretir.
    Clips(ClipsArg),
    /// Aday aralıklarından SRT veya WebVTT altyazı üretir.
    Srt(SrtArg),
    /// Yalnızca ses başlığını ve metadata alanlarını gösterir.
    Info(InfoArg),
}

#[derive(Debug, Args)]
struct ScanArg {
    /// WAV dosya yolu.
    dosya: PathBuf,
    /// Raporun yazılacağı yol. Verilmezse standart çıktıya yazılır.
    #[arg(short = 'o', long)]
    cikti: Option<PathBuf>,
    /// Enerji penceresi (ms). Varsayılan 20.
    #[arg(long, default_value_t = 20)]
    pencere_ms: u32,
    /// Konuşma eşiği: taban gürültünün kaç dB üstü. Varsayılan 10.
    #[arg(long, default_value_t = 10.0)]
    esik_db: f64,
    /// Mutlak alt konuşma eşiği (dBFS). Varsayılan -60.
    #[arg(long, default_value_t = -60.0)]
    en_alcak_db: f64,
    /// "İlk açılış" kuralının aradığı pencere (saniye). Varsayılan 15.
    #[arg(long, default_value_t = 15.0)]
    ilk_acilis_sn: f64,
    /// Tepe için gereken ortalama üstü fark (dB). Varsayılan 6.
    #[arg(long, default_value_t = 6.0)]
    tepe_esik_db: f64,
    /// "Uzun sessizlik" için gereken en az süre (saniye). Varsayılan 2.
    #[arg(long, default_value_t = 2.0)]
    sessizlik_sn: f64,
    /// En yüksek enerji penceresinin genişliği (saniye). Varsayılan 20.
    #[arg(long, default_value_t = 20.0)]
    pencere_sn: f64,
    /// En çok kaç aday döndürüleceği. Varsayılan 30.
    #[arg(long, default_value_t = 30)]
    maks_aday: usize,
}

#[derive(Debug, Args)]
struct ClipsArg {
    /// WAV dosya yolu.
    dosya: PathBuf,
    /// Kırpma planının yazılacağı yol. Verilmezse standart çıktıya yazılır.
    #[arg(short = 'o', long)]
    cikti: Option<PathBuf>,
    /// Tüm adaylar için uygulanacak zaman kaydırması (saniye).
    #[arg(long, default_value_t = 0.0)]
    kaydirma_sn: f64,
    /// Yalnızca bu sıra numaralı adaylar kırpılır (tekrar edilebilir).
    #[arg(long, value_delimiter = ',')]
    sira: Vec<usize>,
    /// Enerji penceresi (ms).
    #[arg(long, default_value_t = 20)]
    pencere_ms: u32,
}

#[derive(Debug, Args)]
struct SrtArg {
    /// WAV dosya yolu.
    dosya: PathBuf,
    /// Altyazının yazılacağı yol. Verilmezse standart çıktıya yazılır.
    #[arg(short = 'o', long)]
    cikti: Option<PathBuf>,
    /// Altyazı biçimi: `srt` veya `vtt`.
    #[arg(short, long, default_value = "srt")]
    bicim: String,
    /// Yalnızca bu sıra numaralı adaylar kullanılır (tekrar edilebilir).
    #[arg(long, value_delimiter = ',')]
    sira: Vec<usize>,
    /// Altyazı üretilen en uzun aday sınırı (saniye). 0 = sınır yok.
    #[arg(long, default_value_t = 0.0)]
    maks_sure_sn: f64,
}

#[derive(Debug, Args)]
struct InfoArg {
    /// WAV dosya yolu.
    dosya: PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match calistir(&cli.komut) {
        Ok(()) => ExitCode::SUCCESS,
        Err(hata) => {
            eprintln!("hata: {hata}");
            ExitCode::FAILURE
        }
    }
}

fn calistir(komut: &Komut) -> Sonuc<()> {
    match komut {
        Komut::Scan(a) => scan(a),
        Komut::Clips(a) => clips(a),
        Komut::Srt(a) => srt(a),
        Komut::Info(a) => info(a),
    }
}

fn enerji_ayari(pencere_ms: u32, esik_db: f64, en_alcak_db: f64) -> EnerjiAyar {
    EnerjiAyar {
        pencere_ms: pencere_ms.max(1),
        esik_carpan_db: esik_db,
        en_alcak_esik_db: en_alcak_db,
        birlestirme_ara_ms: 200,
    }
}

fn kural_ayari() -> KuralAyar {
    KuralAyar::default()
}

fn dosya_adi(yol: &Path) -> String {
    yol.file_name()
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_else(|| yol.to_string_lossy().to_string())
}

fn yaz_veya_yazdir(cikti: &Option<PathBuf>, icerik: &str) -> Sonuc<()> {
    match cikti {
        Some(yol) => {
            let mut dosya = std::fs::File::create(yol)?;
            dosya.write_all(icerik.as_bytes())?;
            eprintln!("yazildi: {}", yol.display());
        }
        None => {
            let mut stdout = std::io::stdout();
            stdout.write_all(icerik.as_bytes())?;
            stdout.write_all(b"\n")?;
        }
    }
    Ok(())
}

fn scan(a: &ScanArg) -> Sonuc<()> {
    let baslik = baslik_oku(&a.dosya)?;
    let mut okuyucu = WavOkuyucu::ac(&a.dosya)?;
    let ayar = enerji_ayari(a.pencere_ms, a.esik_db, a.en_alcak_db);
    let zarf = zarf_uret(&mut okuyucu, &ayar)?;

    let kural = KuralAyar {
        ilk_acilis_pencere_sn: a.ilk_acilis_sn.max(0.0),
        tepe_esigi_db: a.tepe_esik_db,
        uzun_sessizlik_sn: a.sessizlik_sn.max(0.0),
        pencere_suresi_sn: a.pencere_sn.max(0.1),
        maks_aday: a.maks_aday,
        ..kural_ayari()
    };
    let mut adaylar = rules::adaylar_uret(&zarf, &kural);
    rules::sureleri_kirp(&mut adaylar, &kural);
    adaylar.truncate(a.maks_aday);

    let rapor = Rapor::olustur(
        dosya_adi(&a.dosya),
        SesOzeti::basliktan(&baslik),
        &zarf,
        a.pencere_ms,
        adaylar,
        &kural,
    );
    yaz_veya_yazdir(&a.cikti, &rapor.metin()?)
}

fn clips(a: &ClipsArg) -> Sonuc<()> {
    let baslik = baslik_oku(&a.dosya)?;
    let mut okuyucu = WavOkuyucu::ac(&a.dosya)?;
    let ayar = enerji_ayari(a.pencere_ms, kural_ayari().tepe_esigi_db, -60.0);
    let zarf = zarf_uret(&mut okuyucu, &ayar)?;
    let kural = kural_ayari();
    let mut adaylar = rules::adaylar_uret(&zarf, &kural);
    rules::sureleri_kirp(&mut adaylar, &kural);
    adaylar.truncate(kural.maks_aday);
    let adaylar = siraya_gore_suz(&adaylar, &a.sira);
    let plan = KirpmaPlani::olustur(
        &dosya_adi(&a.dosya),
        baslik.sure_sn(),
        a.kaydirma_sn,
        &adaylar,
    );
    yaz_veya_yazdir(&a.cikti, &plan.metin()?)
}

/// `sira` boşsa listeyi olduğu gibi döndürür; doluysa 1 tabanlı sıra
/// numaralarına göre süzer. Adaylar zaten güven puanına göre sıralıdır, bu
/// yüzden sıra numarası listedeki konumdur.
fn siraya_gore_suz(adaylar: &[waveclip::Aday], sira: &[usize]) -> Vec<waveclip::Aday> {
    if sira.is_empty() {
        return adaylar.to_vec();
    }
    adaylar
        .iter()
        .enumerate()
        .filter(|(i, _)| sira.contains(&(i + 1)))
        .map(|(_, ad)| ad.clone())
        .collect()
}

fn srt(a: &SrtArg) -> Sonuc<()> {
    let mut okuyucu = WavOkuyucu::ac(&a.dosya)?;
    let ayar = enerji_ayari(20, kural_ayari().tepe_esigi_db, -60.0);
    let zarf = zarf_uret(&mut okuyucu, &ayar)?;
    let kural = kural_ayari();
    let mut adaylar = rules::adaylar_uret(&zarf, &kural);
    rules::sureleri_kirp(&mut adaylar, &kural);
    let adaylar = siraya_gore_suz(&adaylar, &a.sira);

    let altyazi = AltyaziAyar::default();
    let mut ipuclari = Vec::new();
    for aday in &adaylar {
        let bolgeler = konusma_bolgeleri(&zarf, aday.baslangic_sn, aday.bitis_sn);
        let tepe = bolge_tepe(&zarf, aday.baslangic_sn, aday.bitis_sn);
        let mut uretilen =
            subtitle::ipuclari_uret(aday.baslangic_sn, aday.bitis_sn, tepe, &bolgeler, &altyazi);
        if a.maks_sure_sn > 0.0 {
            let son = aday.baslangic_sn + a.maks_sure_sn;
            uretilen.retain(|i| i.baslangic_sn < son);
        }
        ipuclari.extend(uretilen);
    }
    ipuclari.sort_by(|a, b| {
        a.baslangic_sn
            .partial_cmp(&b.baslangic_sn)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let icerik = match a.bicim.to_ascii_lowercase().as_str() {
        "srt" => subtitle::srt_yaz(&ipuclari),
        "vtt" => subtitle::vtt_yaz(&ipuclari),
        diger => {
            return Err(waveclip::Hata::Ayarlar(format!(
                "desteklenmeyen altyazi bicimi: {diger} (srt veya vtt)"
            )))
        }
    };
    yaz_veya_yazdir(&a.cikti, &icerik)
}

fn konusma_bolgeleri(zarf: &waveclip::EnerjiZarfi, bas: f64, bitis: f64) -> Vec<(f64, f64)> {
    let bolgeler = zarf.konusma_bolgeleri(5);
    bolgeler
        .into_iter()
        .map(|b| {
            (
                zarf.kare_zamani(b.bas).max(bas),
                zarf.kare_zamani(b.bitis).min(bitis),
            )
        })
        .filter(|(b, s)| s > b)
        .collect()
}

fn bolge_tepe(zarf: &waveclip::EnerjiZarfi, bas: f64, bitis: f64) -> f64 {
    let b = zarf.kare_indeksi(bas);
    let s = zarf.kare_indeksi(bitis);
    let s = if s <= b { b + 1 } else { s };
    let b = std::cmp::min(b, zarf.rms_db.len());
    let s = std::cmp::min(s, zarf.rms_db.len());
    if s <= b {
        return zarf.ortalama_db();
    }
    zarf.rms_db[b..s]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
}

fn info(a: &InfoArg) -> Sonuc<()> {
    let baslik = baslik_oku(&a.dosya)?;
    let mut metin = String::new();
    let _ = writeln!(metin, "dosya           : {}", a.dosya.display());
    let _ = writeln!(
        metin,
        "bicim           : PCM (wFormatTag = {})",
        baslik.bicim_kodu
    );
    let _ = writeln!(metin, "ornekleme_hizi  : {} Hz", baslik.ornekleme_hizi);
    let _ = writeln!(metin, "kanal_sayisi    : {}", baslik.kanal_sayisi);
    let _ = writeln!(metin, "bit_derinligi   : {}", baslik.bit_derinligi);
    let _ = writeln!(metin, "blok_hizasi     : {} bayt", baslik.blok_hizasi);
    let _ = writeln!(metin, "veri_boyutu     : {} bayt", baslik.veri_boyutu);
    let _ = writeln!(metin, "kare_sayisi     : {}", baslik.kare_sayisi());
    let _ = writeln!(metin, "sure_sn         : {:.3}", baslik.sure_sn());
    if baslik.meta.is_empty() {
        let _ = writeln!(metin, "meta            : (yok)");
    } else {
        for alan in &baslik.meta {
            let _ = writeln!(metin, "meta.{} = {}", alan.kimlik, alan.deger);
        }
    }
    yaz_veya_yazdir(&None, metin.trim_end())
}
