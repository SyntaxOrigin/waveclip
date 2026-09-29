//! Kural tabanlı hook (ilginç an) adayı üretimi.
//!
//! Sorumluluk: enerji zarfından aday aralıkları çıkarmak, kuralları
//! birbirinden bağımsız çalıştırmak, örtüşen adayları birleştirmek ve
//! tekrarları bastırmak.
//!
//! Sorumluluk dışı: transkripsiyon. Raporda (b07) "soru başlangıcı" ve
//! "konuşmacı değişimi" kuralları kelime zaman damgalarına dayanır; kelime
//! damgası üretilmediği için bu iki kural **kapsam dışıdır** ve raporun
//! önerdiği kural tablosundan geriye kalan dört kural uygulanır.

use serde::{Deserialize, Serialize};

use crate::energy::EnerjiZarfi;

/// Bir adayı hangi kuralın tetiklediğini belirten etiket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kural {
    /// Kaydın ilk N saniyesinde enerjinin yükseldiği ilk konuşma bölgesi.
    /// Raporda "soğuk açılış".
    IlkAcilis,
    /// Kayıt ortalamasının belirgin biçimde üstüne çıkan yerel enerji tepesi.
    EnerjiTepe,
    /// Uzun bir sessizliğin ardından başlayan konuşma.
    SessizlikSonrasiGiris,
    /// Kayıttaki en yüksek enerjiye sahip sabit genişlikli pencere.
    EnYuksekEnerjiPenceresi,
}

impl Kural {
    /// Kuralın rapordaki karşılığı (gerekçe metninde kullanılır).
    pub fn ad(self) -> &'static str {
        match self {
            Kural::IlkAcilis => "ilk acilis",
            Kural::EnerjiTepe => "enerji tepe noktasi",
            Kural::SessizlikSonrasiGiris => "uzun sessizlik sonrasi giris",
            Kural::EnYuksekEnerjiPenceresi => "en yuksek enerji penceresi",
        }
    }

    /// Raporda tanımlanan varsayılan ağırlık sınıfı (ölçülmemiştir, b07 uyarısı).
    pub fn varsayilan_agirlik(self) -> &'static str {
        match self {
            Kural::IlkAcilis => "orta",
            Kural::EnerjiTepe => "yuksek",
            Kural::SessizlikSonrasiGiris => "orta",
            Kural::EnYuksekEnerjiPenceresi => "yuksek",
        }
    }

    /// Bu kuralın verilen tepeye verdiği taban güven puanı.
    ///
    /// Değerler bilinçli olarak 0.45-0.65 bandında tutulur: birleştirme sırasında
    /// puanlar toplanıp 1.0'a dayanırsa sıralama bilgisi kaybolur. Rapor b07
    /// uyarısı gereği bu ağırlıklar ölçülmemiş, gözle seçilmiş başlangıç
    /// değerleridir.
    pub fn taban_guven(self) -> f64 {
        match self {
            Kural::IlkAcilis => 0.50,
            Kural::EnerjiTepe => 0.55,
            Kural::SessizlikSonrasiGiris => 0.45,
            Kural::EnYuksekEnerjiPenceresi => 0.60,
        }
    }
}

/// Bir aday aralığı (saniye cinsinden başlangıç/bitiş) ve puanı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aday {
    /// Adayın başlangıcı (saniye).
    pub baslangic_sn: f64,
    /// Adayın bitişi (saniye, hariç).
    pub bitis_sn: f64,
    /// Güven puanı `0.0..=1.0`.
    pub guven: f64,
    /// Bu adayı üreten kurallar (birden fazla olabilir, adaylar birleşmiştir).
    pub kurallar: Vec<Kural>,
    /// Kullanıcıya gösterilecek yazılı gerekçe.
    pub gerekce: String,
}

impl Aday {
    /// Adayın süresi (saniye).
    pub fn sure_sn(&self) -> f64 {
        (self.bitis_sn - self.baslangic_sn).max(0.0)
    }

    /// İki aday aralığı örtüşüyor mu? ` tolerans_sn` kadar yakınlık da
    /// örtüşme sayılır (bitişmekte olan adaylar tek adaya döner).
    pub fn ortusuyor_veya_yakin(&self, diger: &Aday, tolerans_sn: f64) -> bool {
        self.baslangic_sn < diger.bitis_sn + tolerans_sn
            && diger.baslangic_sn < self.bitis_sn + tolerans_sn
    }

    /// Aynı kural kümesini taşıyan iki adayın tekrarlanabilir (gürültülü)
    /// sayılması için gereken minimum ayrılık (saniye).
    pub const TEKRAR_ESIGI_SN: f64 = 1.0;
}

/// Kural motoru ayarları.
#[derive(Debug, Clone, PartialEq)]
pub struct KuralAyar {
    /// "İlk açılış" kuralının aradığı pencere (saniye).
    pub ilk_acilis_pencere_sn: f64,
    /// Tepe, ortalamadan en az bu kadar dB yüksek olmalı.
    pub tepe_esigi_db: f64,
    /// "En yüksek enerji penceresi" kuralının pencere genişliği (saniye).
    pub pencere_suresi_sn: f64,
    /// "Uzun sessizlik" için gereken en az sessizlik süresi (saniye).
    pub uzun_sessizlik_sn: f64,
    /// Aday süresinin alt sınırı (saniye). Bundan kısa adaylar elenir.
    pub min_aday_sn: f64,
    /// Aday süresinin üst sınırı (saniye). Bundan uzun adaylar kırpılır.
    pub maks_aday_sn: f64,
    /// İki adayın örtüşme sayılması için gereken yakınlık (saniye).
    pub birlestirme_toleransi_sn: f64,
    /// En fazla kaç aday döndürüleceği.
    pub maks_aday: usize,
    /// Enerji zarfındaki en az konuşma bölgesi süresi (saniye).
    pub min_konusma_sn: f64,
}

impl Default for KuralAyar {
    fn default() -> Self {
        KuralAyar {
            ilk_acilis_pencere_sn: 15.0,
            tepe_esigi_db: 6.0,
            pencere_suresi_sn: 20.0,
            uzun_sessizlik_sn: 2.0,
            min_aday_sn: 3.0,
            maks_aday_sn: 45.0,
            birlestirme_toleransi_sn: 0.5,
            maks_aday: 30,
            min_konusma_sn: 0.5,
        }
    }
}

/// Enerji zarfından aday listesi üretir.
///
/// Adaylar önce kuralların ham çıktıları olarak toplanır, sonra örtüşenler
/// birleştirilir, tekrarlar bastırılır ve en güçlü `maks_aday` kadarı
/// güven puanına göre sıralanıp döndürülür.
pub fn adaylar_uret(zarf: &EnerjiZarfi, ayar: &KuralAyar) -> Vec<Aday> {
    if zarf.rms_db.is_empty() {
        return Vec::new();
    }
    let mut ham: Vec<Aday> = Vec::new();
    ham.extend(ilk_acilis(zarf, ayar));
    ham.extend(enerji_tepe(zarf, ayar));
    ham.extend(sessizlik_sonrasi_giris(zarf, ayar));
    ham.extend(en_yuksek_enerji(zarf, ayar));

    let birlestirilmis = birlestir(ham, ayar.birlestirme_toleransi_sn);
    let tekarsiz = tekrarlari_bastir(birlestirilmis);
    let mut sirali = tekarsiz;
    // Güven puanları birbirine yakın olabilir; eşitlikte en erken başlayan
    // aday öne geçer. Sıralama kararlıdır ve tekrarlanabilir.
    sirali.sort_by(|a, b| {
        b.guven
            .partial_cmp(&a.guven)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.baslangic_sn
                    .partial_cmp(&b.baslangic_sn)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    sirali.truncate(ayar.maks_aday);
    sirali
}

/// "Enerji tepe noktası" kuralında bir tepenin "ilginç" sayılma derecesi, kendi
/// çevresindeki ortalamaya göre yükselmesiyle ölçülür. Bu çevre iki saniyedir:
/// 0.5-1 saniyelik vurguları yakalar, ama bir bölümün tamamını ortalama içinde
/// eritmez.
const TEPE_CEVRE_SN: f64 = 2.0;

/// Kural 1 — "soğuk açılış": ilk N saniyede enerjinin yükseldiği ilk bölge.
///
/// Ayarın bu kural üzerindeki etkisini tek başına ölçmek için `pub` tutulur.
pub fn ilk_acilis(zarf: &EnerjiZarfi, ayar: &KuralAyar) -> Vec<Aday> {
    let son_kare = zarf.kare_indeksi(ayar.ilk_acilis_pencere_sn);
    let bolgeler = zarf.konusma_bolgeleri(min_kare(zarf, ayar.min_konusma_sn));
    let ilk = match bolgeler.iter().find(|b| b.bas < son_kare) {
        Some(b) => b,
        None => return Vec::new(),
    };
    let bas = ilk.bas;
    let bitis = std::cmp::min(ilk.bitis, son_kare + kare(zarf, ayar.pencere_suresi_sn));
    let tepe_db = bolge_tepe_db(zarf, bas, bitis);
    vec![Aday {
        baslangic_sn: zarf.kare_zamani(bas),
        bitis_sn: zarf.kare_zamani(bitis),
        guven: Kural::IlkAcilis.taban_guven(),
        kurallar: vec![Kural::IlkAcilis],
        gerekce: format!(
            "ilk {:.0} saniyede enerji yukselen bolge (tepe {:.1} dBFS)",
            ayar.ilk_acilis_pencere_sn, tepe_db
        ),
    }]
}

/// Kural 2 — "enerji tepe noktası": yerel tepe, ortalamayı belirgin aşar.
///
/// Ayarın bu kural üzerindeki etkisini tek başına ölçmek için `pub` tutulur;
/// `adaylar_uret` her zaman birleştirilmiş sonucu döndürür.
pub fn enerji_tepe(zarf: &EnerjiZarfi, ayar: &KuralAyar) -> Vec<Aday> {
    let ortalama = zarf.ortalama_db();
    let esik = (ortalama + ayar.tepe_esigi_db).max(zarf.konusma_esigi_db);
    let tepe_db = zarf.tepe_db();
    if tepe_db < esik {
        return Vec::new();
    }
    let n = zarf.rms_db.len();
    // Yerel çevre genişliği: tepe, kendi çevresine göre ne kadar yükseliyorsa
    // o kadar "ilginç" sayılır. Kayıt ortalaması tek başına yetersizdir: uzun
    // sessizlikli bir kayıtta tüm konuşma bölgeleri ortalamadan çok yüksek
    // olduğu için global ortalama hiçbirini ayırt etmez.
    let cevre = kare(zarf, TEPE_CEVRE_SN).max(1);
    let mut adaylar = Vec::new();
    let mut i = 1usize;
    while i + 1 < n {
        if zarf.rms_db[i] >= esik
            && zarf.rms_db[i] > zarf.rms_db[i - 1]
            && zarf.rms_db[i] >= zarf.rms_db[i + 1]
        {
            let bas = i.saturating_sub(kare(zarf, 0.5));
            let bitis = std::cmp::min(n, i + kare(zarf, 0.5));
            let cevre_bas = i.saturating_sub(cevre);
            let cevre_bit = std::cmp::min(n, i + cevre);
            let yerel_ort = bolge_ortalama_db(zarf, cevre_bas, cevre_bit);
            // Tepe kendi iki saniyelik çevresinden ne kadar yükseltiyorsa
            // puan o kadar artar. Bonus 24 dB'de doyar; podcast genlik aralığı
            // için bu üst sınır pratikte yeterlidir.
            let yukselme = (zarf.rms_db[i] - yerel_ort).clamp(0.0, 24.0);
            let guven = Kural::EnerjiTepe.taban_guven() + 0.30 * yukselme / 24.0;
            adaylar.push(Aday {
                baslangic_sn: zarf.kare_zamani(bas),
                bitis_sn: zarf.kare_zamani(bitis),
                guven,
                kurallar: vec![Kural::EnerjiTepe],
                gerekce: format!(
                    "enerji tepesi {:.1} dBFS, cevre ortalamasi {:.1} dBFS, kayit ortalamasi {:.1} dBFS",
                    zarf.rms_db[i], yerel_ort, ortalama
                ),
            });
            // Aynı tepe çevresindeki komşu tepe noktaları bastırılır.
            i += kare(zarf, 2.0).max(1);
        } else {
            i += 1;
        }
    }
    adaylar
}

/// Kural 3 — "konuşmacı değişimi" yerine geçen "uzun sessizlik sonrası giriş".
///
/// Kelime damgası olmadığı için yön değişimi ölçülemez; yalnızca uzun
/// sessizlik + konuşma başlangıcı sinyaline bakılır.
///
/// Ayarın bu kural üzerindeki etkisini tek başına ölçmek için `pub` tutulur.
pub fn sessizlik_sonrasi_giris(zarf: &EnerjiZarfi, ayar: &KuralAyar) -> Vec<Aday> {
    let bolgeler = zarf.konusma_bolgeleri(min_kare(zarf, ayar.min_konusma_sn));
    if bolgeler.len() < 2 {
        return Vec::new();
    }
    let mut adaylar = Vec::new();
    for pencere in bolgeler.windows(2) {
        let onceki = pencere[0];
        let sonraki = pencere[1];
        let sessizlik_sn = zarf.kare_zamani(sonraki.bas) - zarf.kare_zamani(onceki.bitis);
        if sessizlik_sn < ayar.uzun_sessizlik_sn {
            continue;
        }
        // 10 saniyelik sessizlik bonusun doğduğu noktadır; daha uzun boşluklar
        // puanı daha fazla artırmaz çünkü 10 sn üstü zaten "kesin bölüm sınırı"
        // anlamına gelir ve aşırı puanlamak adayları eşdeğerleştirir.
        let guven =
            Kural::SessizlikSonrasiGiris.taban_guven() + 0.30 * sessizlik_sn.min(10.0) / 10.0;
        adaylar.push(Aday {
            baslangic_sn: zarf.kare_zamani(sonraki.bas),
            bitis_sn: zarf.kare_zamani(std::cmp::min(
                sonraki.bitis,
                sonraki.bas + kare(zarf, ayar.pencere_suresi_sn),
            )),
            guven,
            kurallar: vec![Kural::SessizlikSonrasiGiris],
            gerekce: format!("{sessizlik_sn:.1} saniyelik sessizlikten sonra yeni giris"),
        });
    }
    adaylar
}

/// Kural 4 — "en yüksek enerji penceresi": kaydın en gürültülü sabit penceresi.
///
/// Ayarın bu kural üzerindeki etkisini tek başına ölçmek için `pub` tutulur.
pub fn en_yuksek_enerji(zarf: &EnerjiZarfi, ayar: &KuralAyar) -> Vec<Aday> {
    let pencere = kare(zarf, ayar.pencere_suresi_sn);
    let n = zarf.rms.len();
    if n == 0 {
        return Vec::new();
    }
    if pencere >= n {
        // Kayıt pencere süresinden kısa: tüm kayıt tek pencere olur.
        let tepe = zarf.tepe_db();
        if tepe < zarf.konusma_esigi_db {
            return Vec::new();
        }
        let ortalama = ortalama_taban_db(zarf);
        let fark = (tepe - ortalama).clamp(0.0, 60.0);
        return vec![Aday {
            baslangic_sn: 0.0,
            bitis_sn: zarf.sure_sn(),
            guven: Kural::EnYuksekEnerjiPenceresi.taban_guven() + 0.30 * fark / 60.0,
            kurallar: vec![Kural::EnYuksekEnerjiPenceresi],
            gerekce: format!("kaydin tamami tek pencere, tepe {tepe:.1} dBFS"),
        }];
    }
    let mut en_iyi = 0usize;
    let mut en_iyi_rms = f64::NEG_INFINITY;
    for bas in 0..=(n - pencere) {
        let o = zarf.ortalama_rms(bas, bas + pencere);
        if o > en_iyi_rms {
            en_iyi_rms = o;
            en_iyi = bas;
        }
    }
    let tepe_db = bolge_tepe_db(zarf, en_iyi, en_iyi + pencere);
    if tepe_db < zarf.konusma_esigi_db {
        return Vec::new();
    }
    // Pencere ortalaması ne kadar yüksekse puan o kadar artar: 60 dB taban
    // farkı tam bonus verir, altında doğrusal olarak artar.
    let fark = (bolge_ortalama_db(zarf, en_iyi, en_iyi + pencere) - ortalama_taban_db(zarf))
        .clamp(0.0, 60.0);
    let guven = Kural::EnYuksekEnerjiPenceresi.taban_guven() + 0.30 * fark / 60.0;
    vec![Aday {
        baslangic_sn: zarf.kare_zamani(en_iyi),
        bitis_sn: zarf.kare_zamani(en_iyi + pencere),
        guven,
        kurallar: vec![Kural::EnYuksekEnerjiPenceresi],
        gerekce: format!(
            "en yuksek enerjili {:.0} saniyelik pencere, tepe {:.1} dBFS",
            ayar.pencere_suresi_sn, tepe_db
        ),
    }]
}

/// Bir kare aralığının ortalama RMS'i (dB).
fn bolge_ortalama_db(zarf: &EnerjiZarfi, bas: usize, bitis: usize) -> f64 {
    EnerjiZarfi::rms_to_db(zarf.ortalama_rms(bas, bitis))
}

/// Zarfın ortalama RMS'i (dB).
fn ortalama_taban_db(zarf: &EnerjiZarfi) -> f64 {
    zarf.ortalama_db()
}
/// İki adayın güven puanlarını birleştirir.
///
/// Toplama yerine **doygunluk ekleme** kullanılır: `a + b*(1-a)`. Böylece ikinci
/// bir kural güveni yükseltir ama puan 1.0'a asla dayanmaz; sıralama bilgisi
/// korunur ve tek kurallı bir aday, aynı aralıktaki çok kurallı adaydan daha
/// yüksek puan alamaz.
pub fn birlestirilmis_guven(mevcut: f64, diger: f64) -> f64 {
    (mevcut + diger * (1.0 - mevcut)).min(1.0)
}

/// Örtüşen (veya `tolerans_sn` kadar yakın) adayları tek adayda birleştirir.
///
/// Birleşmede güven puanları doygunluk kuralıyla artar (bkz.
/// [`birlestirilmis_guven`]), kurallar benzersiz sırayla birleşir, gerekçe
/// metni noktalı virgülle eklenir.
pub fn birlestir(adaylar: Vec<Aday>, tolerans_sn: f64) -> Vec<Aday> {
    if adaylar.is_empty() {
        return Vec::new();
    }
    let mut sirali = adaylar;
    sirali.sort_by(|a, b| {
        a.baslangic_sn
            .partial_cmp(&b.baslangic_sn)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut sonuc: Vec<Aday> = Vec::new();
    let mut mevcut = sirali[0].clone();
    for aday in sirali.into_iter().skip(1) {
        if mevcut.ortusuyor_veya_yakin(&aday, tolerans_sn) {
            mevcut.bitis_sn = mevcut.bitis_sn.max(aday.bitis_sn);
            mevcut.guven = birlestirilmis_guven(mevcut.guven, aday.guven);
            for k in aday.kurallar {
                if !mevcut.kurallar.contains(&k) {
                    mevcut.kurallar.push(k);
                }
            }
            if !mevcut.gerekce.contains(&aday.gerekce) {
                mevcut.gerekce = format!("{}; {}", mevcut.gerekce, aday.gerekce);
            }
        } else {
            sonuc.push(mevcut);
            mevcut = aday;
        }
    }
    sonuc.push(mevcut);
    sonuc
}

/// Aynı kural kümesini taşıyan, çok yakın duran adayları bastırır.
///
/// İkinci aday atılır; aynı anda aynı anda iki adayın kullanıcıya iki ayrı
/// "ilginç an" olarak sunulması gürültüdür.
pub fn tekrarlari_bastir(adaylar: Vec<Aday>) -> Vec<Aday> {
    let mut sonuc: Vec<Aday> = Vec::new();
    for aday in adaylar {
        let tekrarli = sonuc.iter().any(|m| {
            m.kurallar == aday.kurallar
                && (m.baslangic_sn - aday.baslangic_sn).abs() < Aday::TEKRAR_ESIGI_SN
        });
        if !tekrarli {
            sonuc.push(aday);
        }
    }
    sonuc
}

/// Aday sürelerini istenen aralığa kırpır; süresiz adayı eler.
pub fn sureleri_kirp(adaylar: &mut Vec<Aday>, ayar: &KuralAyar) {
    adaylar.retain(|a| a.sure_sn() >= ayar.min_aday_sn);
    for a in adaylar.iter_mut() {
        let hedef = a.sure_sn().min(ayar.maks_aday_sn);
        a.bitis_sn = a.baslangic_sn + hedef;
    }
}

fn kare(zarf: &EnerjiZarfi, sure_sn: f64) -> usize {
    if zarf.kare_sure_sn <= 0.0 {
        return 0;
    }
    // `sure_sn` negatif olabilir; `as usize` negatif f64'i 0'a çevirir, bu
    // yüzden ayrıca `.max(0)` gerekmez.
    (sure_sn / zarf.kare_sure_sn).round() as usize
}

fn min_kare(zarf: &EnerjiZarfi, sure_sn: f64) -> usize {
    kare(zarf, sure_sn).max(1)
}

fn bolge_tepe_db(zarf: &EnerjiZarfi, bas: usize, bitis: usize) -> f64 {
    let b = std::cmp::min(bas, zarf.rms_db.len());
    let s = std::cmp::min(bitis, zarf.rms_db.len());
    if s <= b {
        return zarf.ortalama_db();
    }
    zarf.rms_db[b..s]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::{EnerjiAyar, EnerjiToplayici};

    /// Enerji zarfı doğrudan sırayla kurar (testlerin hızı için).
    fn zarf(ornekleme_hizi: u32, kare_sure_sn: f64, db: &[f64]) -> EnerjiZarfi {
        let rms_db = db.to_vec();
        let rms: Vec<f64> = rms_db.iter().map(|d| 10f64.powf(d / 20.0)).collect();
        EnerjiZarfi {
            ornekleme_hizi,
            kare_sure_sn,
            rms,
            rms_db,
            konusma_esigi_db: -45.0,
            taban_gurultu_db: -60.0,
            birlestirme_ara_kare: 3,
        }
    }

    /// Test kaydı (20 ms kare, 50 kare/sn):
    ///
    /// ```text
    /// 0.0-5.0 sn  sessiz
    /// 5.0-6.0 sn  konuşma  (-20 dB)   <- ilk açılış
    /// 6.0-9.0 sn  uzun sessizlik (3 sn)
    /// 9.0-11.0 sn yüksek enerji (-10 dB) <- tepe + sessizlik sonrası giriş
    /// 11.0-15.0 sn uzun sessizlik
    /// 15.0-16.0 sn orta enerji (-18 dB)
    /// ```
    ///
    /// Toplam 800 kare = 16 sn. Varsayılan 20 sn'lik "en yüksek enerji"
    /// penceresi kayıttan uzun olduğu için tam kayıt seçilir.
    fn ornek_kayit() -> EnerjiZarfi {
        let mut db: Vec<f64> = vec![-80.0; 250];
        db.extend(vec![-20.0; 50]);
        db.extend(vec![-80.0; 150]);
        db.extend(vec![-10.0; 100]);
        db.extend(vec![-80.0; 200]);
        db.extend(vec![-18.0; 50]);
        zarf(50, 0.02, &db)
    }

    /// Aynı kayıt ama 30 sn'e uzatılmış: 20 sn'lik pencere kayıt içine sığar,
    /// böylece "en yüksek enerji penceresi" kuralı gerçek pencere seçebilir.
    fn uzun_kayit() -> EnerjiZarfi {
        let mut db: Vec<f64> = vec![-80.0; 250]; // 0-5 sn sessiz
        db.extend(vec![-20.0; 50]); // 5-6 sn konuşma
        db.extend(vec![-80.0; 150]); // 6-9 sn sessizlik
        db.extend(vec![-10.0; 100]); // 9-11 sn yüksek enerji
        db.extend(vec![-80.0; 200]); // 11-15 sn sessizlik
        db.extend(vec![-18.0; 50]); // 15-16 sn orta enerji
        db.extend(vec![-80.0; 700]); // 16-30 sn sessiz
        zarf(50, 0.02, &db)
    }

    #[test]
    fn bos_zarf_adan_uretilmez() {
        let z = zarf(50, 0.02, &[]);
        assert!(adaylar_uret(&z, &KuralAyar::default()).is_empty());
    }

    #[test]
    fn sessiz_kayit_adan_uretilmez() {
        let z = zarf(50, 0.02, &vec![-90.0; 500]);
        assert!(adaylar_uret(&z, &KuralAyar::default()).is_empty());
    }

    #[test]
    fn ilk_acilis_kurali_tetiklenir() {
        // Kural kendi başına test edilir: birleştirme öncesi ham çıktı.
        let ham = ilk_acilis(&ornek_kayit(), &KuralAyar::default());
        assert_eq!(ham.len(), 1);
        assert!(
            (ham[0].baslangic_sn - 5.0).abs() < 0.05,
            "{}",
            ham[0].baslangic_sn
        );
        assert!((ham[0].bitis_sn - 6.0).abs() < 0.05, "{}", ham[0].bitis_sn);
        assert_eq!(ham[0].kurallar, vec![Kural::IlkAcilis]);
        assert!(ham[0].gerekce.contains("ilk 15 saniye"));
    }

    #[test]
    fn ilk_acilis_kurali_ilk_saniyelerde_konusma_yoksa_tetiklenmez() {
        // İlk 15 saniyede konuşma yok, konuşma 20. saniyede başlıyor.
        let mut db: Vec<f64> = vec![-80.0; 1000];
        db.extend(vec![-20.0; 100]);
        assert!(ilk_acilis(&zarf(50, 0.02, &db), &KuralAyar::default()).is_empty());
    }

    #[test]
    fn enerji_tepe_kurali_tetiklenir() {
        let ham = enerji_tepe(&ornek_kayit(), &KuralAyar::default());
        // Üç gürültülü bant (5-6, 9-11, 15-16 sn) üç yerel tepe üretir.
        assert_eq!(ham.len(), 3, "{ham:?}");
        for a in &ham {
            assert_eq!(a.kurallar, vec![Kural::EnerjiTepe]);
            assert!(a.gerekce.contains("cevre ortalamasi"));
            assert!(a.gerekce.contains("kayit ortalamasi"));
            assert!(a.guven > Kural::EnerjiTepe.taban_guven());
            assert!(a.guven <= 0.85 + 1e-9, "{}", a.guven);
        }
        // Üç bandın her biri tek aday verir; 9-11 sn bandı mutlaka aday olmalı.
        assert!(
            ham.iter().any(|a| (a.baslangic_sn - 9.0).abs() < 0.6),
            "9. sn bandi aday olmali: {ham:?}"
        );
        // Yerel çevre ölçütü, iki saniyelik çevresi sessizlikten oluşan kısa
        // vurguları (5-6 ve 15-16 sn) yüksek puanla ödüllendirir: kısa ve
        // ani vurgu uzun bölgeden daha "ilginç"tir.
        let ani = ham
            .iter()
            .find(|a| (a.baslangic_sn - 15.0).abs() < 0.6)
            .expect("15. sn bandi");
        let uzun = ham
            .iter()
            .find(|a| (a.baslangic_sn - 9.0).abs() < 0.6)
            .expect("9. sn bandi");
        assert!(ani.guven > uzun.guven, "{} !> {}", ani.guven, uzun.guven);
    }

    #[test]
    fn yerel_yukselme_global_ortalamadan_daha_iyi_ayirt_eder() {
        // Aynı genlikte iki bölge: biri tamamen sessizlikle, diğeri yumuşak bir
        // zemin üzerinde. Global ortalamaya bakıldığında ikisi de aynı puanı
        // alırdı; yerel çevre ölçümü sessizlikle çevrili olanı öne çıkarır.
        let mut sessiz_cevreli: Vec<f64> = vec![-80.0; 200];
        sessiz_cevreli.extend(vec![-20.0; 50]);
        sessiz_cevreli.extend(vec![-80.0; 200]);
        sessiz_cevreli.extend(vec![-30.0; 50]); // ikinci bölge: zemin -30 dB
        sessiz_cevreli.extend(vec![-30.0; 200]);
        sessiz_cevreli.extend(vec![-80.0; 200]);
        let z = zarf(50, 0.02, &sessiz_cevreli);
        let ham = enerji_tepe(&z, &KuralAyar::default());
        assert!(ham.len() >= 2, "{ham:?}");

        let sessiz_cevde = ham
            .iter()
            .find(|a| (a.baslangic_sn - 4.0).abs() < 0.6)
            .expect("sessizlikle cevrili bolge");
        let yumusak_cevde = ham
            .iter()
            .find(|a| (a.baslangic_sn - 9.0).abs() < 0.6)
            .expect("yumsek zeminde bolge");
        assert!(
            sessiz_cevde.guven > yumusak_cevde.guven,
            "sessiz cevreli {} , yumusak cevreli {}",
            sessiz_cevde.guven,
            yumusak_cevde.guven
        );
    }

    #[test]
    fn enerji_tepe_kurali_duz_tepde_tetiklenmez() {
        // Sabit seviyeli kayıtta tepe, ortalamadan belirgin yüksek değildir.
        let ham = enerji_tepe(&zarf(50, 0.02, &vec![-20.0; 500]), &KuralAyar::default());
        assert!(ham.is_empty());
    }

    #[test]
    fn sessizlik_sonrasi_giris_kurali_tetiklenir() {
        // 5-6 -> 9-11 arası 3 sn, 9-11 -> 15-16 arası 4 sn: iki uzun sessizlik.
        let ham = sessizlik_sonrasi_giris(&ornek_kayit(), &KuralAyar::default());
        assert_eq!(ham.len(), 2, "{ham:?}");
        assert!(
            (ham[0].baslangic_sn - 9.0).abs() < 0.05,
            "{}",
            ham[0].baslangic_sn
        );
        assert!((ham[0].bitis_sn - 11.0).abs() < 0.05, "{}", ham[0].bitis_sn);
        assert!(
            (ham[1].baslangic_sn - 15.0).abs() < 0.05,
            "{}",
            ham[1].baslangic_sn
        );
        assert!((ham[1].bitis_sn - 16.0).abs() < 0.05, "{}", ham[1].bitis_sn);
        for a in &ham {
            assert_eq!(a.kurallar, vec![Kural::SessizlikSonrasiGiris]);
            assert!(a.gerekce.contains("sessizlikten sonra"));
        }
    }

    #[test]
    fn sessizlik_sonrasi_giris_kisa_boslukta_tetiklenmez() {
        // 0.5 sn'lik boşluk eşiğin (2 sn) altında.
        let mut db: Vec<f64> = vec![-20.0; 100];
        db.extend(vec![-80.0; 25]);
        db.extend(vec![-20.0; 100]);
        assert!(sessizlik_sonrasi_giris(&zarf(50, 0.02, &db), &KuralAyar::default()).is_empty());
    }

    #[test]
    fn en_yuksek_enerji_penceresi_kurali_tetiklenir() {
        let ham = en_yuksek_enerji(&uzun_kayit(), &KuralAyar::default());
        assert_eq!(ham.len(), 1);
        // En yüksek ortalama RMS'i veren 20 sn'lik pencere 9-11 sn bandını içerir.
        assert!(ham[0].baslangic_sn <= 9.0, "{}", ham[0].baslangic_sn);
        assert!(ham[0].bitis_sn >= 11.0, "{}", ham[0].bitis_sn);
        assert!((ham[0].sure_sn() - 20.0).abs() < 0.05);
        // Pencere ortalaması kayıt ortalamasından belirgin yüksek olduğu için
        // puan taban değerin üstünde ama 1.0'ın altındadır.
        assert!(
            ham[0].guven > Kural::EnYuksekEnerjiPenceresi.taban_guven(),
            "{}",
            ham[0].guven
        );
        assert!(ham[0].guven <= 0.90 + 1e-9, "{}", ham[0].guven);
    }

    #[test]
    fn en_yuksek_enerji_penceresi_sessizlikte_tetiklenmez() {
        assert!(
            en_yuksek_enerji(&zarf(50, 0.02, &vec![-90.0; 500]), &KuralAyar::default()).is_empty()
        );
    }

    #[test]
    fn birlestirilmis_adaylar_tek_kurala_donusur() {
        // 16 sn'lik kayıtta 20 sn'lik "en yüksek enerji" penceresi tüm kaydı
        // kapsar; diğer üç kural da bu aralığın içinde kalır ve hepsi
        // tek adayda birleşir.
        let adaylar = adaylar_uret(&ornek_kayit(), &KuralAyar::default());
        assert_eq!(adaylar.len(), 1, "{adaylar:?}");
        assert_eq!(adaylar[0].kurallar.len(), 4);
        assert_eq!(adaylar[0].baslangic_sn, 0.0);
    }

    #[test]
    fn ortusen_adaylar_birlestirilir() {
        let a = Aday {
            baslangic_sn: 0.0,
            bitis_sn: 10.0,
            guven: 0.5,
            kurallar: vec![Kural::IlkAcilis],
            gerekce: "A".to_string(),
        };
        let b = Aday {
            baslangic_sn: 9.0,
            bitis_sn: 20.0,
            guven: 0.6,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "B".to_string(),
        };
        let s = birlestir(vec![a, b], 0.5);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].kurallar.len(), 2);
        assert_eq!(s[0].baslangic_sn, 0.0);
        assert_eq!(s[0].bitis_sn, 20.0);
        assert!(s[0].gerekce.contains(';'));
    }

    #[test]
    fn uzak_adaylar_birlestirilmez() {
        let a = Aday {
            baslangic_sn: 0.0,
            bitis_sn: 10.0,
            guven: 0.5,
            kurallar: vec![Kural::IlkAcilis],
            gerekce: "A".to_string(),
        };
        let b = Aday {
            baslangic_sn: 30.0,
            bitis_sn: 40.0,
            guven: 0.6,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "B".to_string(),
        };
        assert_eq!(birlestir(vec![a, b], 0.5).len(), 2);
    }

    #[test]
    fn tekrarli_aday_bastirilir() {
        let a = Aday {
            baslangic_sn: 5.0,
            bitis_sn: 15.0,
            guven: 0.5,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "A".to_string(),
        };
        let b = Aday {
            baslangic_sn: 5.3,
            bitis_sn: 15.3,
            guven: 0.5,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "A kopyasi".to_string(),
        };
        assert_eq!(tekrarlari_bastir(vec![a, b]).len(), 1);
    }

    #[test]
    fn farkli_kuralli_yakin_adaylar_kalir() {
        let a = Aday {
            baslangic_sn: 5.0,
            bitis_sn: 15.0,
            guven: 0.5,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "A".to_string(),
        };
        let b = Aday {
            baslangic_sn: 5.3,
            bitis_sn: 15.3,
            guven: 0.5,
            kurallar: vec![Kural::IlkAcilis],
            gerekce: "B".to_string(),
        };
        assert_eq!(tekrarlari_bastir(vec![a, b]).len(), 2);
    }

    #[test]
    fn adaylar_guvene_gore_siralanir() {
        let adaylar = adaylar_uret(&ornek_kayit(), &KuralAyar::default());
        for pencere in adaylar.windows(2) {
            assert!(
                pencere[0].guven + 1e-12 >= pencere[1].guven,
                "{:?} sirali degil",
                adaylar
            );
        }
    }

    #[test]
    fn maks_aday_siniri_uygulanir() {
        let ayar = KuralAyar {
            maks_aday: 1,
            ..KuralAyar::default()
        };
        assert_eq!(adaylar_uret(&ornek_kayit(), &ayar).len(), 1);
    }

    #[test]
    fn sure_kirpma_calisir() {
        let mut adaylar = vec![Aday {
            baslangic_sn: 0.0,
            bitis_sn: 100.0,
            guven: 0.5,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "A".to_string(),
        }];
        let ayar = KuralAyar {
            maks_aday_sn: 10.0,
            min_aday_sn: 3.0,
            ..KuralAyar::default()
        };
        sureleri_kirp(&mut adaylar, &ayar);
        assert_eq!(adaylar.len(), 1);
        assert!((adaylar[0].sure_sn() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn kisa_sureli_aday_elenir() {
        let mut adaylar = vec![Aday {
            baslangic_sn: 0.0,
            bitis_sn: 1.0,
            guven: 0.5,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "A".to_string(),
        }];
        let ayar = KuralAyar {
            min_aday_sn: 3.0,
            ..KuralAyar::default()
        };
        sureleri_kirp(&mut adaylar, &ayar);
        assert!(adaylar.is_empty());
    }

    #[test]
    fn pencere_kayittan_uzunsa_tam_kayit_secilir() {
        // 2 saniyelik kayıt, 20 saniyelik pencere isteği.
        let z = zarf(50, 0.02, &vec![-15.0; 100]);
        let adaylar = adaylar_uret(&z, &KuralAyar::default());
        let pencere = adaylar
            .iter()
            .find(|a| a.kurallar.contains(&Kural::EnYuksekEnerjiPenceresi))
            .expect("tam kayit penceresi olmali");
        assert_eq!(pencere.baslangic_sn, 0.0);
        assert!((pencere.bitis_sn - 2.0).abs() < 1e-9);
    }

    #[test]
    fn kural_adi_ve_agirlik_donusu_kararli() {
        assert_eq!(Kural::IlkAcilis.ad(), "ilk acilis");
        assert_eq!(
            Kural::EnYuksekEnerjiPenceresi.varsayilan_agirlik(),
            "yuksek"
        );
        // Taban puanlar 0.45-0.60 bandında tutulur: birleştirme doygunlukla
        // çalıştığı için yüksek tabanlar tek kurallı adayları da 1.0'a
        // dayandırırdı.
        for k in [
            Kural::IlkAcilis,
            Kural::EnerjiTepe,
            Kural::SessizlikSonrasiGiris,
            Kural::EnYuksekEnerjiPenceresi,
        ] {
            assert!(
                (0.45..=0.60).contains(&k.taban_guven()),
                "{:?} taban guveni {}",
                k,
                k.taban_guven()
            );
        }
    }

    #[test]
    fn birlestirilmis_guven_doygunlukla_artar() {
        // 0.6 + 0.6 = 1.2 olmamalı; doygunluk kuralı sonucu 0.84'te tutar.
        let s = birlestirilmis_guven(0.6, 0.6);
        assert!((s - 0.84).abs() < 1e-9, "{s}");
        // Aynı puan ikinci kez eklendiğinde artış küçülür.
        assert!(birlestirilmis_guven(s, 0.6) > s);
        assert!(birlestirilmis_guven(s, 0.6) < 1.0);
        // Girdiler korunur.
        assert!((birlestirilmis_guven(0.5, 0.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn aday_ortusme_mantigi_dogru() {
        let a = Aday {
            baslangic_sn: 0.0,
            bitis_sn: 10.0,
            guven: 0.5,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "A".to_string(),
        };
        let b = Aday {
            baslangic_sn: 10.2,
            bitis_sn: 20.0,
            guven: 0.5,
            kurallar: vec![Kural::EnerjiTepe],
            gerekce: "B".to_string(),
        };
        // 0.2 sn'lik fark, 0.5 toleransta sayılır.
        assert!(a.ortusuyor_veya_yakin(&b, 0.5));
        assert!(!a.ortusuyor_veya_yakin(&b, 0.1));
    }

    #[test]
    fn gercek_zarftan_aday_uretilebilir() {
        // 20 kare sessizlik, 50 kare konuşma, 5 kare nefes, 50 kare konuşma
        let mut t = EnerjiToplayici::yeni(50, EnerjiAyar::default());
        t.ekle(&vec![0.0f32; 400]);
        t.ekle(&vec![0.7f32; 1000]);
        t.ekle(&vec![0.0f32; 100]);
        t.ekle(&vec![0.9f32; 1000]);
        let z = t.bitir();
        let adaylar = adaylar_uret(&z, &KuralAyar::default());
        assert!(!adaylar.is_empty(), "gercek zarf en az bir aday vermeli");
        for a in &adaylar {
            assert!(a.baslangic_sn >= 0.0);
            assert!(a.bitis_sn > a.baslangic_sn);
            assert!((0.0..=1.0).contains(&a.guven));
        }
    }
}
