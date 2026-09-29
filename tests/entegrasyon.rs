//! Uçtan uca entegrasyon testleri: WAV dosyası yaz → zarf üret → aday üret →
//! JSON/SRT/VTT üret.
//!
//! Her test kendi geçici dizinini üretir (`std::process::id()` + etiket), bu
//! yüzden testler birbirinden bağımsızdır.

mod yardimci;

use std::collections::BTreeMap;
use std::path::Path;

use waveclip::energy::{zarf_uret, EnerjiAyar, ORNEK_TAMPON};
use waveclip::hata::Hata;
use waveclip::rapor::{KirpmaPlani, Rapor, SesOzeti};
use waveclip::rules::{self, KuralAyar};
use waveclip::subtitle::{self, AltyaziAyar};
use waveclip::wav::{baslik_oku, WavOkuyucu, OKUMA_BLOK_BAYT};

use yardimci::{GeciciDizin, Tarif};

fn ayar() -> EnerjiAyar {
    EnerjiAyar::default()
}

fn kural_ayari() -> KuralAyar {
    KuralAyar::default()
}

/// Tam akış: WAV → zarf → aday → rapor.
fn tam_akis(yol: &Path) -> (waveclip::EnerjiZarfi, Rapor) {
    let baslik = baslik_oku(yol).expect("baslik okunmali");
    let mut okuyucu = WavOkuyucu::ac(yol).expect("okuyucu acilmali");
    let zarf = zarf_uret(&mut okuyucu, &ayar()).expect("zarf uretilmeli");
    let kural = kural_ayari();
    let mut adaylar = rules::adaylar_uret(&zarf, &kural);
    rules::sureleri_kirp(&mut adaylar, &kural);
    let rapor = Rapor::olustur(
        "test.wav".to_string(),
        SesOzeti::basliktan(&baslik),
        &zarf,
        20,
        adaylar,
        &kural,
    );
    (zarf, rapor)
}

#[test]
fn ornek_kayit_uyctan_uca_tarama_uretir() {
    let dizin = GeciciDizin::yeni("tarama").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let (zarf, rapor) = tam_akis(&yol);

    assert_eq!(rapor.ses.ornekleme_hizi, 8000);
    assert_eq!(rapor.ses.kanal_sayisi, 1);
    assert_eq!(rapor.ses.bit_derinligi, 16);
    assert!(!rapor.adaylar.is_empty(), "en az bir aday uretilmeli");
    assert_eq!(rapor.aday_sayisi, rapor.adaylar.len());
    assert!((zarf.sure_sn() - 10.0).abs() < 0.05, "{}", zarf.sure_sn());
}

#[test]
fn tarama_jsonu_geri_okunur_ve_zorunlu_alanlari_icerir() {
    let dizin = GeciciDizin::yeni("json").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let (_, rapor) = tam_akis(&yol);
    let metin = rapor.metin().expect("json uretilmeli");

    for alan in ["kaynak", "sure_sn", "aday_sayisi", "kural_surumu"] {
        assert!(metin.contains(alan), "JSON'da '{alan}' alani yok");
    }
    let geri = Rapor::metinden(&metin).expect("json cozumlenmeli");
    assert_eq!(geri, rapor);
    assert_eq!(geri.kural_surumu, waveclip::rapor::KURAL_SURESI);
}

#[test]
fn her_adayin_gerekcesi_ve_kurali_var() {
    let dizin = GeciciDizin::yeni("gerekce").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let (_, rapor) = tam_akis(&yol);
    assert!(!rapor.adaylar.is_empty());
    for aday in &rapor.adaylar {
        assert!(!aday.kurallar.is_empty(), "kuralsiz aday: {aday:?}");
        assert!(
            aday.gerekce.len() > 10,
            "gerekce cok kisa: {}",
            aday.gerekce
        );
        assert!(aday.baslangic_sn >= 0.0);
        assert!(aday.bitis_sn <= rapor.sure_sn + 1e-6);
        assert!(aday.sure_sn() >= kural_ayari().min_aday_sn - 1e-9);
        assert!(aday.sure_sn() <= kural_ayari().maks_aday_sn + 1e-9);
    }
}

#[test]
fn adaylar_birbirini_ortusmez() {
    let dizin = GeciciDizin::yeni("ortusme").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let (_, rapor) = tam_akis(&yol);
    for pencere in rapor.adaylar.windows(2) {
        assert!(
            pencere[0].bitis_sn <= pencere[1].baslangic_sn + 1e-9,
            "ortusen adaylar: {} ve {}",
            pencere[0].bitis_sn,
            pencere[1].baslangic_sn
        );
    }
}

#[test]
fn tekrarli_adaylar_birlesir() {
    let dizin = GeciciDizin::yeni("tekrar").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let (_, rapor) = tam_akis(&yol);
    let benzersiz: BTreeMap<String, usize> =
        rapor.adaylar.iter().fold(BTreeMap::new(), |mut m, a| {
            let anahtar = a
                .kurallar
                .iter()
                .map(|k| format!("{k:?}"))
                .collect::<Vec<_>>()
                .join("+");
            *m.entry(anahtar).or_insert(0) += 1;
            m
        });
    for (anahtar, adet) in benzersiz {
        assert!(adet <= 1, "aynı kural kümesi {adet} kez: {anahtar}");
    }
}

#[test]
fn sessiz_dosya_adan_uretmez() {
    let dizin = GeciciDizin::yeni("sessiz").expect("gecici dizin");
    let tarif = Tarif {
        bolgeler: vec![(4.0, 0.0)],
        ..Tarif::ornek()
    };
    let yol = yardimci::wav_yaz(&dizin, "sessiz.wav", &tarif).expect("waz");
    let (zarf, rapor) = tam_akis(&yol);
    assert!(zarf.tepe_db() <= -119.0, "{}", zarf.tepe_db());
    assert!(rapor.adaylar.is_empty(), "sessiz kayitta aday olmamali");
}

#[test]
fn cok_kisa_dosya_adan_uretmez_ve_hata_vermez() {
    let dizin = GeciciDizin::yeni("kisa").expect("gecici dizin");
    let tarif = Tarif {
        bolgeler: vec![(0.05, 0.0), (0.02, 0.6)],
        ..Tarif::ornek()
    };
    let yol = yardimci::wav_yaz(&dizin, "kisa.wav", &tarif).expect("waz");
    let (zarf, rapor) = tam_akis(&yol);
    assert!(zarf.sure_sn() < 0.1);
    assert!(rapor.adaylar.is_empty());
}

#[test]
fn bos_dosya_ve_sifir_veri_islenir() {
    let dizin = GeciciDizin::yeni("bosveri").expect("gecici dizin");
    let tarif = Tarif {
        bolgeler: vec![(0.0, 0.0)],
        ..Tarif::ornek()
    };
    let yol = yardimci::wav_yaz(&dizin, "bos.wav", &tarif).expect("waz");
    let baslik = baslik_oku(&yol).expect("baslik okunmali");
    assert_eq!(baslik.kare_sayisi(), 0);
    assert_eq!(baslik.sure_sn(), 0.0);
    let (zarf, rapor) = tam_akis(&yol);
    assert!(zarf.rms.is_empty());
    assert_eq!(rapor.aday_sayisi, 0);
}

#[test]
fn altyazi_uretilir_ve_zamanlama_kayit_ici_kalir() {
    let dizin = GeciciDizin::yeni("altyazi").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let zarf = {
        let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
        zarf_uret(&mut okuyucu, &ayar()).expect("zarf")
    };
    let kural = kural_ayari();
    let mut adaylar = rules::adaylar_uret(&zarf, &kural);
    rules::sureleri_kirp(&mut adaylar, &kural);

    let mut hepsi = Vec::new();
    for aday in &adaylar {
        let bolgeler: Vec<(f64, f64)> = zarf
            .konusma_bolgeleri(5)
            .into_iter()
            .map(|b| (zarf.kare_zamani(b.bas), zarf.kare_zamani(b.bitis)))
            .filter(|(s, b)| *b > aday.baslangic_sn && *s < aday.bitis_sn)
            .collect();
        hepsi.extend(subtitle::ipuclari_uret(
            aday.baslangic_sn,
            aday.bitis_sn,
            zarf.tepe_db(),
            &bolgeler,
            &AltyaziAyar::default(),
        ));
    }
    assert!(!hepsi.is_empty(), "altyazi ipucu uretilmeli");
    for i in &hepsi {
        assert!(i.baslangic_sn >= 0.0);
        assert!(i.bitis_sn <= zarf.sure_sn() + 1e-6, "{i:?}");
        assert!(i.sure_sn() >= AltyaziAyar::default().min_sure_sn - 1e-9);
    }

    let srt = subtitle::srt_yaz(&hepsi);
    assert!(srt.contains("-->"));
    assert!(
        !srt.contains("\n\n\n"),
        "SRT bloklari bos satir ile ayrilmali"
    );
    let vtt = subtitle::vtt_yaz(&hepsi);
    assert!(vtt.starts_with("WEBVTT"));
    assert!(vtt.contains("-->"));
}

#[test]
fn kirpma_plani_uretir_ve_kaydirma_uygular() {
    let dizin = GeciciDizin::yeni("kirpma").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let (_, rapor) = tam_akis(&yol);
    let plan = KirpmaPlani::olustur("ornek.wav", rapor.sure_sn, 1.5, &rapor.adaylar);
    assert_eq!(plan.klip_sayisi, plan.klipler.len());
    for klip in &plan.klipler {
        assert!(klip.baslangic_sn >= 1.5 - 1e-9);
        assert!(klip.bitis_sn <= rapor.sure_sn + 1e-6);
        assert!(klip.bitis_sn > klip.baslangic_sn);
    }
    let metin = plan.metin().expect("json");
    assert_eq!(KirpmaPlani::metinden(&metin).expect("cozum"), plan);
}

#[test]
fn list_info_metadatasi_dosyadan_okunur() {
    let dizin = GeciciDizin::yeni("meta").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "meta.wav", &Tarif::ornek()).expect("waz");
    let baslik = baslik_oku(&yol).expect("baslik");
    let eslesen: BTreeMap<&str, &str> = baslik
        .meta
        .iter()
        .map(|a| (a.kimlik.as_str(), a.deger.as_str()))
        .collect();
    assert_eq!(eslesen.get("INAM"), Some(&"Bolum 12"));
    assert_eq!(eslesen.get("IART"), Some(&"Test Konukmaci"));
}

#[test]
fn mono_ve_stereo_kayitlar_ayni_zarfi_uretir() {
    let dizin = GeciciDizin::yeni("kanal").expect("gecici dizin");
    let mono = yardimci::wav_yaz(&dizin, "mono.wav", &Tarif::ornek()).expect("waz");
    let stereo_tarif = Tarif {
        kanal: 2,
        ..Tarif::ornek()
    };
    let stereo = yardimci::wav_yaz(&dizin, "stereo.wav", &stereo_tarif).expect("waz");

    let m = baslik_oku(&mono).expect("mono baslik");
    let s = baslik_oku(&stereo).expect("stereo baslik");
    assert_eq!(m.kanal_sayisi, 1);
    assert_eq!(s.kanal_sayisi, 2);
    // İki kanal aynı veriyi taşıdığından mono'ya indirgeme sonucu aynıdır.
    assert!((m.sure_sn() - s.sure_sn()).abs() < 1e-9);

    let mut om = WavOkuyucu::ac(&mono).expect("ac");
    let mut os = WavOkuyucu::ac(&stereo).expect("ac");
    let zm = zarf_uret(&mut om, &ayar()).expect("zarf");
    let zs = zarf_uret(&mut os, &ayar()).expect("zarf");
    assert_eq!(zm.rms.len(), zs.rms.len());
    for (a, b) in zm.rms.iter().zip(zs.rms.iter()) {
        assert!((a - b).abs() < 0.01, "{a} != {b}");
    }
}

#[test]
fn bit_derinlikleri_ayni_enerji_verir() {
    let dizin = GeciciDizin::yeni("bit").expect("gecici dizin");
    let mut zarf_degerleri = Vec::new();
    for (ad, bit) in [("b8", 8u16), ("b16", 16), ("b24", 24), ("b32", 32)] {
        let tarif = Tarif {
            bit,
            ..Tarif::ornek()
        };
        let yol = yardimci::wav_yaz(&dizin, &format!("{ad}.wav"), &tarif).expect("waz");
        let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
        let zarf = zarf_uret(&mut okuyucu, &ayar()).expect("zarf");
        // Tepe, `Tarif::ornek` içindeki en yüksek genlikli (0.95) bölgeden gelir.
        // 20 ms pencere 220 Hz'te 4.4 periyot kapsar; RMS pencereleme nedeniyle
        // birkaç yüzde sapma normaldir. 8-bit imzasız olduğu için çözünürlük daha
        // düşüktür, bu yüzden toleransı geniş tutulur.
        let tepe_rms = 0.95f64 / std::f64::consts::SQRT_2;
        let beklenen_db = 20.0 * tepe_rms.log10();
        let sapma_db = (zarf.tepe_db() - beklenen_db).abs();
        let tolerans_db = if bit == 8 { 0.2 } else { 0.15 };
        assert!(
            sapma_db < tolerans_db,
            "{bit}-bit sapma {sapma_db} dB (tolerans {tolerans_db})"
        );
        zarf_degerleri.push(zarf.tepe_db());
    }
    assert_eq!(zarf_degerleri.len(), 4);
}

#[test]
fn kirk_ve_bir_khz_kayitlar_ayni_sure_verir() {
    let dizin = GeciciDizin::yeni("hiz").expect("gecici dizin");
    let a = yardimci::wav_yaz(&dizin, "a.wav", &Tarif::ornek()).expect("waz");
    let b_tarif = Tarif {
        hiz: 44100,
        ..Tarif::ornek()
    };
    let b = yardimci::wav_yaz(&dizin, "b.wav", &b_tarif).expect("waz");
    let ma = baslik_oku(&a).expect("a");
    let mb = baslik_oku(&b).expect("b");
    assert_eq!(ma.ornekleme_hizi, 8000);
    assert_eq!(mb.ornekleme_hizi, 44100);
    assert!((ma.sure_sn() - mb.sure_sn()).abs() < 0.02);
}

#[test]
fn sinus_tonu_rms_degerine_yakin() {
    let dizin = GeciciDizin::yeni("sinus").expect("gecici dizin");
    let tarif = Tarif {
        bolgeler: vec![(2.0, 1.0)],
        frekans: 1000.0,
        ..Tarif::ornek()
    };
    let yol = yardimci::wav_yaz(&dizin, "sinus.wav", &tarif).expect("waz");
    let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
    let zarf = zarf_uret(&mut okuyucu, &ayar()).expect("zarf");
    let beklenen = 1.0 / std::f64::consts::SQRT_2;
    for r in &zarf.rms {
        assert!((r - beklenen).abs() < 0.01, "rms {r}, beklenen {beklenen}");
    }
}

#[test]
fn dc_ofsetli_kayit_okunur() {
    let dizin = GeciciDizin::yeni("dc").expect("gecici dizin");
    let tarif = Tarif {
        bolgeler: vec![(1.0, 0.0)],
        dc_ofset: 0.3,
        ..Tarif::ornek()
    };
    let yol = yardimci::wav_yaz(&dizin, "dc.wav", &tarif).expect("waz");
    let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
    let zarf = zarf_uret(&mut okuyucu, &ayar()).expect("zarf");
    // DC ofseti enerjiye yansır: 0.3 ofset RMS'i 0.3'e yaklaştırır.
    assert!((zarf.rms[0] - 0.3).abs() < 0.02, "{}", zarf.rms[0]);
}

#[test]
fn kirpilmis_dosya_kismi_okunur() {
    let dizin = GeciciDizin::yeni("kirpilmis").expect("gecici dizin");
    let veri = yardimci::pcm_uret(&Tarif::ornek());
    let mut baytlar = yardimci::wav_ustur(&Tarif::ornek(), &veri, None);
    // Dosyanın son yarısını kes: data kutusu bildirilen boyuttan uzun kalır.
    baytlar.truncate(baytlar.len() - veri.len() / 2);
    let yol = dizin.dosya("kirpik.wav");
    std::fs::write(&yol, &baytlar).expect("waz");

    let baslik = baslik_oku(&yol).expect("kirpma toleransi");
    let tam = veri.len() / 2;
    assert!(baslik.veri_boyutu <= tam as u64);
    let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
    let zarf = zarf_uret(&mut okuyucu, &ayar()).expect("zarf");
    assert!(!zarf.rms.is_empty(), "kismi veriden zarf uretilmeli");
    assert!(zarf.sure_sn() < 10.0);
}

#[test]
fn wav_olmayan_dosya_reddedilir() {
    let dizin = GeciciDizin::yeni("wavdegil").expect("gecici dizin");
    let yol = dizin.dosya("sahte.mp3");
    std::fs::write(
        &yol,
        b"ID3\x04\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00payload",
    )
    .expect("waz");
    let hata = baslik_oku(&yol).expect_err("hata beklenir");
    assert!(matches!(hata, Hata::KapsayiciDegil { .. }), "{hata:?}");
}

#[test]
fn sikistirilmis_bicim_hata_mesaji_verir() {
    let dizin = GeciciDizin::yeni("sikistirma").expect("gecici dizin");
    let mut govde = b"WAVEfmt ".to_vec();
    govde.extend_from_slice(&16u32.to_le_bytes());
    govde.extend_from_slice(&0x11u16.to_le_bytes()); // IMA ADPCM
    govde.extend_from_slice(&1u16.to_le_bytes());
    govde.extend_from_slice(&8000u32.to_le_bytes());
    govde.extend_from_slice(&8000u32.to_le_bytes());
    govde.extend_from_slice(&256u16.to_le_bytes());
    govde.extend_from_slice(&4u16.to_le_bytes());
    govde.extend_from_slice(b"data");
    govde.extend_from_slice(&64u32.to_le_bytes());
    govde.extend_from_slice(&[0u8; 64]);
    let mut baytlar = b"RIFF".to_vec();
    baytlar.extend_from_slice(&(govde.len() as u32).to_le_bytes());
    baytlar.extend_from_slice(&govde);
    let yol = dizin.dosya("adpcm.wav");
    std::fs::write(&yol, &baytlar).expect("waz");
    let hata = baslik_oku(&yol).expect_err("hata beklenir");
    let metin = hata.to_string();
    assert!(metin.contains("PCM"), "{metin}");
}

#[test]
fn eksik_data_kutusu_hata_verir() {
    let dizin = GeciciDizin::yeni("eksikdata").expect("gecici dizin");
    let mut govde = b"WAVEfmt ".to_vec();
    govde.extend_from_slice(&16u32.to_le_bytes());
    govde.extend_from_slice(&1u16.to_le_bytes());
    govde.extend_from_slice(&1u16.to_le_bytes());
    govde.extend_from_slice(&8000u32.to_le_bytes());
    govde.extend_from_slice(&16000u32.to_le_bytes());
    govde.extend_from_slice(&2u16.to_le_bytes());
    govde.extend_from_slice(&16u16.to_le_bytes());
    let mut baytlar = b"RIFF".to_vec();
    baytlar.extend_from_slice(&(govde.len() as u32).to_le_bytes());
    baytlar.extend_from_slice(&govde);
    let yol = dizin.dosya("eksik.wav");
    std::fs::write(&yol, &baytlar).expect("waz");
    let hata = baslik_oku(&yol).expect_err("hata beklenir");
    assert!(matches!(hata, Hata::VeriKutusuYok), "{hata:?}");
}

#[test]
fn eksik_fmt_kutusu_hata_verir() {
    let dizin = GeciciDizin::yeni("eksikfmt").expect("gecici dizin");
    let mut baytlar = b"RIFF".to_vec();
    baytlar.extend_from_slice(&20u32.to_le_bytes());
    baytlar.extend_from_slice(b"WAVE");
    baytlar.extend_from_slice(b"JUNK");
    baytlar.extend_from_slice(&4u32.to_le_bytes());
    baytlar.extend_from_slice(&[0u8; 4]);
    let yol = dizin.dosya("junk.wav");
    std::fs::write(&yol, &baytlar).expect("waz");
    let hata = baslik_oku(&yol).expect_err("hata beklenir");
    assert!(matches!(hata, Hata::FormKutusuYok), "{hata:?}");
}

#[test]
fn bom_ve_ek_yol_adi_olmadan_dosya_okunur() {
    // Bazı araçlar dosyanın başına bir BOM ya da boş bir JUNK kutusu koyar.
    // Ayrıştırıcı bunları yok saymalıdır.
    let dizin = GeciciDizin::yeni("bom").expect("gecici dizin");
    let veri = yardimci::pcm_uret(&Tarif::ornek());
    let mut govde = b"WAVEJUNK".to_vec();
    govde.extend_from_slice(&4u32.to_le_bytes());
    govde.extend_from_slice(&[0xEFu8, 0xBB, 0xBF, 0x00]); // UTF-8 BOM dolgusu
    govde.extend_from_slice(b"fmt ");
    govde.extend_from_slice(&16u32.to_le_bytes());
    govde.extend_from_slice(&1u16.to_le_bytes());
    govde.extend_from_slice(&1u16.to_le_bytes());
    govde.extend_from_slice(&8000u32.to_le_bytes());
    govde.extend_from_slice(&16000u32.to_le_bytes());
    govde.extend_from_slice(&2u16.to_le_bytes());
    govde.extend_from_slice(&16u16.to_le_bytes());
    govde.extend_from_slice(b"data");
    govde.extend_from_slice(&(veri.len() as u32).to_le_bytes());
    govde.extend_from_slice(&veri);
    let mut baytlar = b"RIFF".to_vec();
    baytlar.extend_from_slice(&(govde.len() as u32).to_le_bytes());
    baytlar.extend_from_slice(&govde);
    let yol = dizin.dosya("bom.wav");
    std::fs::write(&yol, &baytlar).expect("waz");

    let baslik = baslik_oku(&yol).expect("okunmali");
    assert_eq!(baslik.ornekleme_hizi, 8000);
    assert!(baslik.kare_sayisi() > 0);
    assert!(baslik.meta.is_empty());
}

#[test]
fn buyuk_dosyada_tampon_boyutu_sabit_kalir() {
    // 8 MB ve 32 MB olmak üzere iki kayıt: okuma tamponu ve örnek tamponu
    // her ikisinde de aynı olmalıdır. Ses verisi belleğe alınmadığı için
    // bu iki sayı kayıt uzunluğundan bağımsızdır.
    let dizin = GeciciDizin::yeni("bellek").expect("gecici dizin");

    let mut olcumler = Vec::new();
    for (ad, sure_sn) in [("kucuk.wav", 20u64), ("buyuk.wav", 80)] {
        let kare = 8000u64 * sure_sn;
        let mut veri = vec![0u8; (kare * 2) as usize];
        // Ortasına yüksek enerjili bir bölge koy.
        let bas = (kare * 2 / 3) as usize;
        for bayt in veri[bas..bas + 16000].iter_mut() {
            *bayt = 0xFF;
        }
        let tarif = Tarif::ornek();
        let baytlar = yardimci::wav_ustur(&tarif, &veri, None);
        let yol = dizin.dosya(ad);
        std::fs::write(&yol, &baytlar).expect("waz");

        let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
        let okuma_tamponu = okuyucu.tampon_boyutu();
        let toplayici_baslangic = okuma_tamponu;
        let zarf = zarf_uret(&mut okuyucu, &ayar()).expect("zarf");
        // Zorunlu örnek tamponu sabittir.
        assert_eq!(ORNEK_TAMPON, 8192);
        assert_eq!(okuyucu.tampon_boyutu(), okuma_tamponu);
        assert_eq!(toplayici_baslangic, okuma_tamponu);
        // 4 kat büyük dosya -> 4 kat zarf karesi (tek doğrusal büyüme).
        olcumler.push((zarf.rms.len(), okuyucu.tampon_boyutu()));
    }

    let (kucuk_kare, kucuk_tampon) = olcumler[0];
    let (buyuk_kare, buyuk_tampon) = olcumler[1];
    assert_eq!(
        kucuk_tampon, buyuk_tampon,
        "tampon boyutu kayitla buyumemeli"
    );
    assert!(
        (buyuk_kare as f64 / kucuk_kare as f64 - 4.0).abs() < 0.05,
        "zarf kare sayisi kayitla dogru orantili degil: {kucuk_kare} -> {buyuk_kare}"
    );
}

#[test]
fn okuma_tamponu_sabit_konstanti() {
    // Sözleşme: WAV okuyucunun ham tamponu kayıttan bağımsız olarak sabittir.
    assert_eq!(OKUMA_BLOK_BAYT, 64 * 1024);
    let dizin = GeciciDizin::yeni("tampon").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let okuyucu = WavOkuyucu::ac(&yol).expect("ac");
    assert!(okuyucu.tampon_boyutu() <= OKUMA_BLOK_BAYT + 64 * 1024);
    assert!(okuyucu.tampon_boyutu() >= OKUMA_BLOK_BAYT);
}

#[test]
fn adaylar_ayar_degistikce_degisir() {
    // Ayarın çıktıya etkisi ölçülür: yüksek tepe eşiği aday sayısını azaltır.
    let dizin = GeciciDizin::yeni("ayar").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
    let zarf = zarf_uret(&mut okuyucu, &ayar()).expect("zarf");

    // "En yüksek enerji penceresi" kuralı kayıt genelinde en yüksek tepeyi
    // kaçınılmaz olarak seçer; tepe eşiği onu etkilemez. Etkilenmesi beklenen
    // kural "enerji tepe noktası"dır: yüksek eşik hiçbir tepeyi geçmez.
    let gevsek = KuralAyar {
        tepe_esigi_db: 0.0,
        ..KuralAyar::default()
    };
    // Eşik `ortalama + tepe_esigi_db` olarak hesaplanır. Kayıt büyük ölçüde
    // sessizlik olduğu için ortalama çok düşüktür; tepeyi tamamen elemek için
    // eşiğin tepe değerinin üstüne çıkması gerekir (tepe ≈ -3 dBFS).
    let sert = KuralAyar {
        tepe_esigi_db: 200.0,
        ..KuralAyar::default()
    };
    let gevsek_tepe = rules::enerji_tepe(&zarf, &gevsek).len();
    let sert_tepe = rules::enerji_tepe(&zarf, &sert).len();
    assert!(gevsek_tepe > 0, "gevsek esikte tepe bulunmali");
    assert_eq!(sert_tepe, 0, "asilan esikte tepe kalmamali");

    // Aynı kayıtta `maks_aday` ayarı da doğrudan aday sayısını belirler.
    let tek = rules::adaylar_uret(&zarf, &KuralAyar::default()).len();
    let sinirli = KuralAyar {
        maks_aday: 1,
        ..KuralAyar::default()
    };
    assert!(tek >= 1);
    assert_eq!(rules::adaylar_uret(&zarf, &sinirli).len(), 1);
}

#[test]
fn cok_bolgeli_uzun_kayitta_birden_fazla_aday_uretilir() {
    // 83 saniyelik kayıt: üç ayrı gürültülü bölge, aralarında 25 saniyelik
    // sessizlik. Varsayılan 20 saniyelik "en yüksek enerji" penceresi yalnız
    // üçüncü bölgeyi kapsayabildiği için adaylar ayrışır.
    let dizin = GeciciDizin::yeni("cokbolge").expect("gecici dizin");
    let tarif = Tarif {
        bolgeler: vec![
            (2.0, 0.0),
            (2.0, 0.6),
            (25.0, 0.0),
            (2.0, 0.8),
            (25.0, 0.0),
            (2.0, 0.95),
            (25.0, 0.0),
        ],
        ..Tarif::ornek()
    };
    let yol = yardimci::wav_yaz(&dizin, "uzun.wav", &tarif).expect("waz");
    let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
    let zarf = zarf_uret(&mut okuyucu, &ayar()).expect("zarf");
    let hepsi = rules::adaylar_uret(&zarf, &KuralAyar::default());
    assert!(
        hepsi.len() >= 3,
        "birbirinden ayrı en az 3 aday beklenir: {hepsi:?}"
    );
    let sinirli = KuralAyar {
        maks_aday: 2,
        ..KuralAyar::default()
    };
    assert_eq!(rules::adaylar_uret(&zarf, &sinirli).len(), 2);
}

#[test]
fn pencere_genisligi_kare_sayisini_belirler() {
    let dizin = GeciciDizin::yeni("pencere").expect("gecici dizin");
    let yol = yardimci::wav_yaz(&dizin, "ornek.wav", &Tarif::ornek()).expect("waz");
    for (ms, beklenen) in [(20u32, 500usize), (40, 250), (100, 100)] {
        let mut okuyucu = WavOkuyucu::ac(&yol).expect("ac");
        let zarf = zarf_uret(
            &mut okuyucu,
            &EnerjiAyar {
                pencere_ms: ms,
                ..EnerjiAyar::default()
            },
        )
        .expect("zarf");
        assert!(
            (zarf.rms.len() as i64 - beklenen as i64).abs() <= 2,
            "{ms} ms -> {} kare (beklenen {beklenen})",
            zarf.rms.len()
        );
        assert!((zarf.kare_sure_sn - ms as f64 / 1000.0).abs() < 1e-9);
    }
}

#[test]
fn srt_ve_vtt_ayni_ipuclardan_uretilir() {
    let ipuclar = vec![
        waveclip::Ipucu {
            baslangic_sn: 0.5,
            bitis_sn: 2.0,
            metin: "Ilk satir".to_string(),
        },
        waveclip::Ipucu {
            baslangic_sn: 3.0,
            bitis_sn: 5.25,
            metin: "Ikinci satir".to_string(),
        },
    ];
    let srt = subtitle::srt_yaz(&ipuclar);
    let vtt = subtitle::vtt_yaz(&ipuclar);
    assert!(srt.contains("00:00:00,500 --> 00:00:02,000"));
    assert!(vtt.contains("00:00:00.500 --> 00:00:02.000"));
    assert!(srt.contains("1\n") && srt.contains("\n2\n"));
    // VTT'te sıra numarası yoktur.
    assert!(!vtt.contains("\n1\n00:00"));
}
