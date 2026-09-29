//! SRT ve WebVTT altyazı üretimi.
//!
//! Sorumluluk: aday aralıklarından segment granülaritesinde altyazı
//! zamanlaması üretmek ve iki biçimi de geçerli biçimde yazmak.
//!
//! Sorumluluk dışı: kelime düzeyinde zamanlama ve altyazı **basma** (burn-in).
//! Kelime damgası üretilmediği için metin, enerji zarfindaki konuşma
//! bölgelerinden türetilen zaman etiketlerinden oluşur. Bu, MANIFEST kart 02'de
//! ertelenen altyazı basma adımının saf metin karşılığıdır.

use std::fmt::Write as _;

/// Tek bir altyazı girdisi.
#[derive(Debug, Clone, PartialEq)]
pub struct Ipucu {
    /// Başlangıç (saniye).
    pub baslangic_sn: f64,
    /// Bitiş (saniye, hariç).
    pub bitis_sn: f64,
    /// Altyazı metni.
    pub metin: String,
}

impl Ipucu {
    /// Süre (saniye).
    pub fn sure_sn(&self) -> f64 {
        (self.bitis_sn - self.baslangic_sn).max(0.0)
    }
}

/// Altyazı üretim ayarları.
#[derive(Debug, Clone, PartialEq)]
pub struct AltyaziAyar {
    /// En kısa gösterim süresi (saniye). Daha kısası okunamaz.
    pub min_sure_sn: f64,
    /// En uzun gösterim süresi (saniye).
    pub maks_sure_sn: f64,
    /// Ardışık iki ipucu arasındaki en küçük boşluk (saniye).
    pub min_bosluk_sn: f64,
    /// Bir ipucuna yazılacak en uzun metin (karakter).
    pub maks_karakter: usize,
}

impl Default for AltyaziAyar {
    fn default() -> Self {
        AltyaziAyar {
            min_sure_sn: 1.0,
            maks_sure_sn: 7.0,
            min_bosluk_sn: 0.08,
            maks_karakter: 42,
        }
    }
}

/// Bir adayın konuşma bölgelerinden altyazı ipuçları üretir.
///
/// Her konuşma bölgesi bir ipucuna dönüşür; metin, bölgenin başlangıç zamanı
/// ve enerji tepe değerinden türetilir. Aşırı uzun bölgeler `maks_sure_sn`
/// parçalarına bölünür.
pub fn ipuclari_uret(
    bas: f64,
    bitis: f64,
    tepe_db: f64,
    bolgeler: &[(f64, f64)],
    ayar: &AltyaziAyar,
) -> Vec<Ipucu> {
    let mut ipuclari: Vec<Ipucu> = Vec::new();
    for (b_bas, b_bitis) in bolgeler {
        let mut parca_bas = *b_bas;
        while parca_bas + ayar.min_sure_sn <= *b_bitis {
            let parca_bitis = (parca_bas + ayar.maks_sure_sn).min(*b_bitis);
            let metin = format!("[{}] tepe {:.1} dBFS", saniye_etiket(*b_bas), tepe_db);
            let metin = if metin.chars().count() > ayar.maks_karakter {
                metin.chars().take(ayar.maks_karakter).collect()
            } else {
                metin
            };
            ipuclari.push(Ipucu {
                baslangic_sn: parca_bas,
                bitis_sn: parca_bitis,
                metin,
            });
            parca_bas = parca_bitis;
        }
    }
    normalize(&mut ipuclari, bas, bitis, ayar);
    ipuclari
}

/// Sıfır uzunluklı ve çakışan ipuçlarını düzeltir; sıralama ve kırpma uygular.
fn normalize(ipuclari: &mut Vec<Ipucu>, bas: f64, bitis: f64, ayar: &AltyaziAyar) {
    // Başlangıç sırasına göre düzenle.
    ipuclari.sort_by(|a, b| {
        a.baslangic_sn
            .partial_cmp(&b.baslangic_sn)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // Kayıt aralığının dışına taşan ve minimum süresi olmayan ipuçlarını ele.
    ipuclari.retain(|i| i.sure_sn() >= ayar.min_sure_sn && i.baslangic_sn < bitis);
    for i in ipuclari.iter_mut() {
        i.baslangic_sn = i.baslangic_sn.clamp(bas, bitis);
        i.bitis_sn = i.bitis_sn.clamp(bas, bitis);
    }
    // Ardışık ipuçların çakışmasını, korunan en az boşlukla çöz.
    for i in 1..ipuclari.len() {
        let en_erken = ipuclari[i - 1].bitis_sn + ayar.min_bosluk_sn;
        ipuclari[i].baslangic_sn = ipuclari[i].baslangic_sn.max(en_erken);
    }
    // Çakışma çözümü minimum süreyi bozduysa ipucu düşer.
    ipuclari.retain(|i| i.bitis_sn > i.baslangic_sn && i.sure_sn() >= ayar.min_sure_sn);
}

/// Saniyeyi `HH:MM:SS.mmm` biçimine çevirir (SRT ve VTT'nin ortak gösterimi).
pub fn zaman_bicimle(saniye: f64) -> String {
    let toplam_ms = (saniye.max(0.0) * 1000.0).round() as u64;
    let saat = toplam_ms / 3_600_000;
    let dakika = (toplam_ms / 60_000) % 60;
    let saniye_k = (toplam_ms / 1000) % 60;
    let ms = toplam_ms % 1000;
    format!("{saat:02}:{dakika:02}:{saniye_k:02}.{ms:03}")
}

/// SRT gösterimi: virgüllü milisaniye ayracı kullanır.
pub fn srt_zaman(saniye: f64) -> String {
    zaman_bicimle(saniye).replace('.', ",")
}

/// Saniyeyi kısa `0:07` biçimine çevirir (altyazı metni etiketi).
pub fn saniye_etiket(saniye: f64) -> String {
    let toplam = saniye.max(0.0).round() as u64;
    format!("{}:{:02}", toplam / 60, toplam % 60)
}

/// SRT dosya içeriğini üretir.
///
/// Biçim: 1'den başlayan sıra numarası, `HH:MM:SS,mmm --> HH:MM:SS,mmm`,
/// metin, boş satır.
pub fn srt_yaz(ipuclari: &[Ipucu]) -> String {
    let mut cikti = String::new();
    for (i, ipucu) in ipuclari.iter().enumerate() {
        let _ = writeln!(cikti, "{}", i + 1);
        let _ = writeln!(
            cikti,
            "{} --> {}",
            srt_zaman(ipucu.baslangic_sn),
            srt_zaman(ipucu.bitis_sn)
        );
        let _ = writeln!(cikti, "{}", ipucu.metin);
        cikti.push('\n');
    }
    cikti
}

/// WebVTT dosya içeriğini üretir.
///
/// `WEBVTT` başlığı ve noktalı milisaniye ayracı kullanılır.
pub fn vtt_yaz(ipuclari: &[Ipucu]) -> String {
    let mut cikti = String::from("WEBVTT\n\n");
    for ipucu in ipuclari {
        let _ = writeln!(
            cikti,
            "{} --> {}",
            zaman_bicimle(ipucu.baslangic_sn),
            zaman_bicimle(ipucu.bitis_sn)
        );
        let _ = writeln!(cikti, "{}", ipucu.metin);
        cikti.push('\n');
    }
    cikti
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zaman_bicimle_dogru() {
        assert_eq!(zaman_bicimle(0.0), "00:00:00.000");
        assert_eq!(zaman_bicimle(1.5), "00:00:01.500");
        assert_eq!(zaman_bicimle(61.25), "00:01:01.250");
        assert_eq!(zaman_bicimle(3661.0), "01:01:01.000");
    }

    #[test]
    fn negatif_zaman_sifira_kirpilir() {
        assert_eq!(zaman_bicimle(-5.0), "00:00:00.000");
        assert_eq!(srt_zaman(-1.0), "00:00:00,000");
    }

    #[test]
    fn srt_zamani_virgul_ayiracli() {
        assert_eq!(srt_zaman(3.25), "00:00:03,250");
    }

    #[test]
    fn saniye_etiketi_kisa_bicim() {
        assert_eq!(saniye_etiket(0.0), "0:00");
        assert_eq!(saniye_etiket(7.4), "0:07");
        assert_eq!(saniye_etiket(125.0), "2:05");
        assert_eq!(saniye_etiket(-3.0), "0:00");
    }

    #[test]
    fn ipuclari_uret_konusalma_bolgelerinden() {
        let bolgeler = [(0.0, 3.0), (5.0, 8.0)];
        let ipuclari = ipuclari_uret(0.0, 10.0, -18.2, &bolgeler, &AltyaziAyar::default());
        assert_eq!(ipuclari.len(), 2);
        assert_eq!(ipuclari[0].baslangic_sn, 0.0);
        assert!(ipuclari[0].metin.contains("-18.2"));
        assert!(ipuclari[0].metin.contains("0:00"));
    }

    #[test]
    fn ipuclari_uret_cok_uzun_bolgeyi_boler() {
        let bolgeler = [(0.0, 30.0)];
        let ipuclari = ipuclari_uret(0.0, 30.0, -10.0, &bolgeler, &AltyaziAyar::default());
        assert!(ipuclari.len() >= 4, "{}", ipuclari.len());
        for i in &ipuclari {
            assert!(i.sure_sn() <= AltyaziAyar::default().maks_sure_sn + 1e-9);
        }
    }

    #[test]
    fn cok_kisa_bolge_ipucu_uretmez() {
        let bolgeler = [(0.0, 0.4)];
        let ipuclari = ipuclari_uret(0.0, 10.0, -10.0, &bolgeler, &AltyaziAyar::default());
        assert!(ipuclari.is_empty());
    }

    #[test]
    fn normalize_cakisan_baslangiclari_ayirir() {
        let mut i = vec![
            Ipucu {
                baslangic_sn: 0.0,
                bitis_sn: 5.0,
                metin: "a".to_string(),
            },
            Ipucu {
                baslangic_sn: 1.0,
                bitis_sn: 7.0,
                metin: "b".to_string(),
            },
        ];
        normalize(&mut i, 0.0, 10.0, &AltyaziAyar::default());
        assert_eq!(i.len(), 2, "ikinci ipucu minimum sureyi korumali");
        let ayar = AltyaziAyar::default();
        assert!(i[1].baslangic_sn >= i[0].bitis_sn + ayar.min_bosluk_sn - 1e-9);
    }

    #[test]
    fn normalize_kayit_suresini_asmaz() {
        let mut i = vec![Ipucu {
            baslangic_sn: 8.0,
            bitis_sn: 20.0,
            metin: "a".to_string(),
        }];
        normalize(&mut i, 0.0, 10.0, &AltyaziAyar::default());
        assert!(i[0].bitis_sn <= 10.0 + 1e-9);
    }

    #[test]
    fn normalize_sure_sonundaki_ipucu_elenir() {
        let mut i = vec![Ipucu {
            baslangic_sn: 12.0,
            bitis_sn: 15.0,
            metin: "a".to_string(),
        }];
        normalize(&mut i, 0.0, 10.0, &AltyaziAyar::default());
        assert!(i.is_empty());
    }

    #[test]
    fn metin_uzunlugu_kirpilir() {
        let bolgeler = [(0.0, 5.0)];
        let ipuclari = ipuclari_uret(0.0, 10.0, -10.0, &bolgeler, &AltyaziAyar::default());
        for i in &ipuclari {
            assert!(i.metin.chars().count() <= AltyaziAyar::default().maks_karakter);
        }
    }

    #[test]
    fn srt_yaz_dogru_yapi() {
        let ipuclari = vec![
            Ipucu {
                baslangic_sn: 0.0,
                bitis_sn: 2.5,
                metin: "Merhaba".to_string(),
            },
            Ipucu {
                baslangic_sn: 3.0,
                bitis_sn: 5.0,
                metin: "Dunya".to_string(),
            },
        ];
        let s = srt_yaz(&ipuclari);
        let beklenen = "1\n00:00:00,000 --> 00:00:02,500\nMerhaba\n\n2\n00:00:03,000 --> 00:00:05,000\nDunya\n\n";
        assert_eq!(s, beklenen);
    }

    #[test]
    fn vtt_yaz_dogru_yapi() {
        let ipuclari = vec![Ipucu {
            baslangic_sn: 1.0,
            bitis_sn: 2.0,
            metin: "Test".to_string(),
        }];
        let s = vtt_yaz(&ipuclari);
        assert!(s.starts_with("WEBVTT\n\n"));
        assert!(s.contains("00:00:01.000 --> 00:00:02.000"));
        assert!(s.contains("Test"));
    }

    #[test]
    fn bos_liste_srt_bos_vtt_baslikli() {
        assert_eq!(srt_yaz(&[]), "");
        assert_eq!(vtt_yaz(&[]), "WEBVTT\n\n");
    }
}
