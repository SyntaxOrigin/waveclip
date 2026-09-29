# DalgaKes (WaveClip)

Podcast WAV kayıtlarından kural tabanlı **hook** (ilginç an) adayı bulan ve bu
adaylardan SRT / WebVTT altyazı üreten yerel komut satırı aracı.

Tamamen çevrimdışı çalışır; ağ yığını derleme dışında hiçbir yerde kullanılmaz.
Ses dosyasının tamamı belleğe alınmaz: RIFF/WAVE kapsayıcısı akış hâlinde
okunur, yalnızca 20 ms'lik pencerelerin RMS enerji serisi tutulur.

## Özellikler

MVP kapsamındaki her özellik:

- **WAV RIFF ayrıştırıcı** — `RIFF`/`WAVE` kapsayıcısı, `fmt ` ve `data` kutuları,
  `LIST`/`INFO` metadata okuma, bilinmeyen kutuların atlanması, RIFF hizalama
  dolgusunun (pad byte) doğru ele alınması.
- **PCM genişlikleri** — 8-bit (WAV'te **imzasız**), 16-bit, 24-bit (işaret
  uzantısıyla 32-bit'e genişletilir) ve 32-bit PCM. Kanal başına 8 kHz ile
  192 kHz arası her örnekleme hızı; mono ve stereo (kanallar ortalama alınarak
  mono'ya indirgenir).
- **Akış tabanlı okuyucu** — 64 KiB sabit okuma tamponu. `data` kutusu
  bildirimden büyükse (kırpılmış dosya) mevcut baytla sınırlanır, çökmez.
- **Segment başına RMS enerji zarfı** — sabit pencere (varsayılan 20 ms), tek
  geçişli, düşük bellek. Kısmi son pencere de yazılır.
- **Konuşma/sessizlik ayrımı** — eşik taban gürültünün üstüne kurulur; taban
  gürültü RMS dağılımının alt yüzdeliğinden tahmin edilir. Mutlak alt eşik
  (varsayılan −60 dBFS) sayısallaştırma gürültüsünü "konuşma" saymaz.
- **Dört kural tabanlı hook adayı:**
  1. `ilk-acilis` — kaydın ilk N saniyesinde (varsayılan 15 sn) enerjinin
     yükseldiği ilk konuşma bölgesi (rapor b07 "soğuk açılış").
  2. `enerji-tepe` — kayıt ortalamasını aşan, iki saniyelik çevresine göre
     belirgin biçimde yükselen yerel tepe noktaları.
  3. `sessizlik-sonrasi-giris` — uzun bir sessizliğin (varsayılan ≥ 2 sn)
     ardından başlayan konuşma.
  4. `en-yuksek-enerji-penceresi` — kaydın en gürültülü sabit genişlikli
     penceresi.
- **Örtüşen adayların birleştirilmesi** — aynı aralığa düşen adaylar tek
  adayda birleşir, kuralları ve gerekçeleri birleşir, güven puanları doygunluk
  kuralıyla (`a + b·(1−a)`) artar.
- **Tekrarların bastırılması** — aynı kural kümesini taşıyan ve 1 saniyeden
  yakın duran adaylar elenir (gürültülü aday bastırma).
- **SRT ve WebVTT altyazı üretimi** — segment granülaritesinde, minimum 1.0 sn
  gösterim, 80 ms zorunlu boşluk, kayıt süresi sınırı, 42 karakter sınırı.
- **JSON çıktı** — `kaynak`, `sure_sn`, `aday_sayisi`, `kural_surumu` alanları
  zorunlu; aday listesi başlangıç/bitiş, güven skoru, tetikleyen kurallar ve
  yazılı gerekçe taşır.
- **Kırpma planı** — `--kaydirma-sn` ile zaman kaydırma uygulanmış, kayıt
  aralığına kırpılmış klip listesi (JSON).
- **Komut satırı** — `clap` ile `scan`, `clips`, `srt`, `info` alt komutları.

## Kurulum

Gereksinim: Rust **1.74+** (MSRV, `Cargo.toml`'de `rust-version` olarak sabit).
Bu depoda geliştirme ve doğrulama **Rust 1.98.1** ile yapıldı.

```powershell
cargo build --release
```

Gerçek çıktı:

```
    Finished `release` profile [optimized] target(s) in 14.24s
```

İkili dosya `target\release\waveclip.exe` altına üretilir.

Doğrudan çalıştırma (derleme zincirinden):

```powershell
cargo run --release -- info .\ornek.wav
```

```
     Running `target\release\waveclip.exe info .\ornek.wav`
dosya           : .\ornek.wav
bicim           : PCM (wFormatTag = 1)
ornekleme_hizi  : 44100 Hz
kanal_sayisi    : 1
bit_derinligi   : 16
blok_hizasi     : 2 bayt
veri_boyutu     : 7938000 bayt
kare_sayisi     : 3969000
sure_sn         : 90.000
meta            : (yok)
```

Yerel kurulum (`cargo install`):

```powershell
cargo install --path .
```

```
  Installing %USERPROFILE%\.cargo\bin\waveclip.exe
   Installed package `waveclip v0.1.0 (%USERPROFILE%\Desktop\Projeler\projects\02-waveclip)` (executable `waveclip.exe`)
```

Kurulumdan sonra `waveclip` komutu doğrudan PATH'te bulunur. Aşağıdaki
`## Kullanım` örneklerinin tamamı bu kurulumla, `ornek.wav` dosyasının bulunduğu
klasörde çalıştırılmıştır.

## Kullanım

Örnekler `ornek.wav` adlı, 44.1 kHz / 16-bit / mono, 90 saniyelik sentetik bir
PCM kayıt üzerinde gerçekten çalıştırılmıştır. Kayıt şu yapıdadır:
0–8 sn sessizlik, 8–20 sn orta enerji, 20–40 sn sessizlik, 40–56 sn yüksek
enerji, 56–70 sn sessizlik, 70–86 sn orta enerji, 86–90 sn sessizlik.

### `info` — ses başlığını görmek

```powershell
waveclip info .\ornek.wav
```

```
dosya           : ornek.wav
bicim           : PCM (wFormatTag = 1)
ornekleme_hizi  : 44100 Hz
kanal_sayisi    : 1
bit_derinligi   : 16
blok_hizasi     : 2 bayt
veri_boyutu     : 7938000 bayt
kare_sayisi     : 3969000
sure_sn         : 90.000
meta            : (yok)
```

`LIST`/`INFO` metadata içeren bir dosyada:

```powershell
waveclip info .\meta.wav
```

```
dosya           : meta.wav
bicim           : PCM (wFormatTag = 1)
ornekleme_hizi  : 44100 Hz
kanal_sayisi    : 1
bit_derinligi   : 16
blok_hizasi     : 2 bayt
veri_boyutu     : 7938000 bayt
kare_sayisi     : 3969000
sure_sn         : 90.000
meta.INAM = Bolum 12 - DalgaKes
meta.IART = Test Konukmaci
meta.ICRD = 2026
meta.ISFT = PowerShell ornek ureticisi
```

### `scan` — hook adaylarını bulmak

```powershell
waveclip scan .\ornek.wav -o .\rapor.json
```

```
yazildi: .\rapor.json
```

`-o` verilmezse JSON standart çıktıya yazılır. Raporun içeriği (kısaltılmış):

```json
{
  "kaynak": "ornek.wav",
  "sure_sn": 90.0,
  "aday_sayisi": 3,
  "kural_surumu": "waveclip-kural-1.0.0",
  "ses": {
    "bicim_kodu": 1,
    "kanal_sayisi": 1,
    "ornekleme_hizi": 44100,
    "bit_derinligi": 16,
    "ozet": "PCM 44100 Hz, 1 kanal, 16 bit",
    "meta": []
  },
  "enerji": {
    "pencere_ms": 20,
    "kare_sayisi": 4500,
    "taban_gurultu_db": -100.0,
    "konusma_esigi_db": -60.0,
    "ortalama_db": -64.12749937620731,
    "tepe_db": -3.4561002799779645
  },
  "adaylar": [
    {
      "baslangic_sn": 36.0,
      "bitis_sn": 56.0,
      "guven": 0.9900380321772151,
      "kurallar": ["en-yuksek-enerji-penceresi", "enerji-tepe", "sessizlik-sonrasi-giris"],
      "gerekce": "en yuksek enerjili 20 saniyelik pencere, tepe -3.5 dBFS; enerji tepesi -3.5 dBFS, cevre ortalamasi -9.5 dBFS, kayit ortalamasi -64.1 dBFS; 20.0 saniyelik sessizlikten sonra yeni giris"
    },
    {
      "baslangic_sn": 69.5,
      "bitis_sn": 86.0,
      "guven": 0.9063143747289989,
      "kurallar": ["enerji-tepe", "sessizlik-sonrasi-giris"],
      "gerekce": "enerji tepesi -6.1 dBFS, cevre ortalamasi -12.1 dBFS, kayit ortalamasi -64.1 dBFS; 14.0 saniyelik sessizlikten sonra yeni giris"
    },
    {
      "baslangic_sn": 7.5,
      "bitis_sn": 20.0,
      "guven": 0.8126287494579976,
      "kurallar": ["enerji-tepe", "ilk-acilis"],
      "gerekce": "enerji tepesi -8.2 dBFS, cevre ortalamasi -14.2 dBFS, kayit ortalamasi -64.1 dBFS; ilk 15 saniyede enerji yukselen bolge (tepe -8.2 dBFS)"
    }
  ]
}
```

Adaylar güven puanına göre sıralanır; en güçlü aday (40–56 sn bandındaki yüksek
enerjili bölge) başta gelir.

### `clips` — kırpma planı üretmek

```powershell
waveclip clips .\ornek.wav -o .\plan.json --kaydirma-sn 2.5
```

```
yazildi: .\plan.json
```

Plan, tüm adaylara 2.5 saniyelik kaydırma uygulanmış ve kayıt aralığına
kırpılmış hâlde döner:

```json
{
  "kaynak": "ornek.wav",
  "kural_surumu": "waveclip-kural-1.0.0",
  "kaydirma_sn": 2.5,
  "sure_sn": 90.0,
  "klip_sayisi": 3,
  "klipler": [
    { "baslangic_sn": 10.0, "bitis_sn": 22.5, "guven": 0.8126287494579976, "...": "..." },
    { "baslangic_sn": 38.5, "bitis_sn": 58.5, "guven": 0.9900380321772151, "...": "..." },
    { "baslangic_sn": 72.0, "bitis_sn": 88.5, "guven": 0.9063143747289989, "...": "..." }
  ]
}
```

Yalnızca belirli adayları seçmek için `--sira` kullanılır (1 tabanlı, virgülle
ayrılmış, tekrarlanabilir).

### `srt` — altyazı üretmek

```powershell
waveclip srt .\ornek.wav -o .\ornek.srt --sira 1
```

```
yazildi: .\ornek.srt
```

```text
1
00:00:40,000 --> 00:00:47,000
[0:40] tepe -3.5 dBFS

2
00:00:47,080 --> 00:00:54,000
[0:40] tepe -3.5 dBFS

3
00:00:54,080 --> 00:00:56,000
[0:40] tepe -3.5 dBFS
```

WebVTT için:

```powershell
waveclip srt .\ornek.wav -b vtt --sira 2
```

```
WEBVTT

00:01:10.000 --> 00:01:17.000
[1:10] tepe -6.1 dBFS

00:01:17.080 --> 00:01:24.000
[1:10] tepe -6.1 dBFS

00:01:24.080 --> 00:01:26.000
[1:10] tepe -6.1 dBFS
```

### Yardım çıktısı

```powershell
waveclip --help
```

```
Usage: waveclip.exe <COMMAND>

Commands:
  scan   Bir WAV dosyasını tarayıp hook adaylarını JSON olarak raporlar
  clips  Onaylanan adaylardan kırpma planı (JSON) üretir
  srt    Aday aralıklarından SRT veya WebVTT altyazı üretir
  info   Yalnızca ses başlığını ve metadata alanlarını gösterir
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help (see a summary with '-h')
  -V, --version  Print version
```

## Test

```powershell
cargo test
```

Gerçek çıktı (Rust 1.98.1, Windows):

```
running 79 tests
test result: ok. 79 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 28 tests
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Toplam: okunan 107; geçen 107; başarısız 0** (79 birim + 28 entegrasyon).

Diğer kalite kapıları:

```powershell
cargo build --release          # hatasız, uyarısız
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Üçü de bu depoda temiz geçmektedir.

### Test kapsamı ve kenar durumları

Kapsanan kenar durumları (her biri en az bir testle doğrulanır):

| Alan | Kapsanan durumlar |
|---|---|
| RIFF başlığı | `RIFF` imzası yok, `WAVE` imzası yok, 12 bayttan kısa başlık, bilinmeyen kutular, BOM/dolgu içeren kutular, `fmt ` kutusu 16 bayttan kısa, `data` kutusu yok |
| Bozuk/kırpılmış kutu | `data` boyutu dosya sonundan taşıyor → kısmi kabul, `fmt ` ve `data` arası eksik kutular |
| Bit genişliği | 8-bit **imzasız** (128 = 0), 16-bit, 24-bit (işaret uzantısı), 32-bit, 4-bit reddi, kısa tampon |
| Kanal / hız | mono ve stereo aynı zarfa indirgenir, 8 kHz ve 44.1 kHz aynı süreyi verir, kanal sayısı 0 reddi |
| Boş / kısa / sessiz | `data` kutusu boş, 0.07 sn'lik kayıt, tamamen sıfır içeren dosya, kısmi pencere |
| Sinyal | tam genlikli sabit sinyal, 1 kHz sinüs (RMS = 0.707), DC ofseti, DC ofsetli sinüs |
| Altyazı zamanlaması | negatif zaman sıfıra kırpılır, saat taşması (`01:01:01.000`), SRT virgül ayracı, minimum süre, çakışan ipuçlarının ayrılması, kayıt süresini aşan ipucunun elenmesi, metin uzunluk kırpma |
| Hook kuralları | her kuralın tek başına tetiklenmesi, tetiklenmeme koşulu, pencere kayıttan uzunsa tam kayıt seçimi, sesizlikte hiç aday üretmemesi |
| Aday seçimi | örtüşen adayların birleşmesi, uzak adayların birleşmemesi, aynı kural kümesindeki tekrarların bastırılması, farklı kural kümesindeki yakın adayların kalması, adayların örtüşmemesi, `maks_aday` sınırı, kısa adayın elenmesi |
| Ayarların etkisi | tepe eşiğinin aday sayısını düşürmesi, pencere genişliğinin kare sayısını belirlemesi, `maks_aday` sınırı |
| Büyük dosyada bellek | 20 sn ve 80 sn'lik iki kayıtta okuma tamponu ve örnek tamponu **aynı** kalır; zarf kare sayısı kayıt uzunluğuyla tam orantılıdır |
| JSON | zorunlu alanların varlığı, gidiş–dönüş (bit bit aynı `f64`), bozuk JSON, kırpma planı gidiş–dönüşü |

**Sabit bellek iddiasının doğrulanması.** İki ayrı ölçüm vardır:

1. `wav::WavOkuyucu::tampon_boyutu()` ham okuma tamponunu (64 KiB) ve blok
   hizası için ayrılan alanı döndürür. `buyuk_dosyada_tampon_boyutu_sabit_kalir`
   testi 20 sn ve 80 sn'lik iki kaydı aynı işlemde okur ve iki değerin **birebir
   eşit** olduğunu doğrular; ayrıca zarf kare sayısının tam 4 katına çıktığını
   (20 sn → 1000 kare, 80 sn → 4000 kare) kontrol eder.
2. `energy::EnerjiToplayici::aktif_tampon_boyutu()` kısmi pencerede bekleyen
   örnek sayısını döndürür ve bu değer her zaman `kare_ornek - 1`'den küçüktür
   (`aktif_tampon_sabit_kalir`).

Sonuç: ses verisi hiçbir zaman tümüyle belleğe alınmaz; kayıt uzunluğuyla
büyüyen tek yapı pencere sonuçlarıdır (`Vec<f64>`, 8 bayt/kare). 60 dakikalık
44.1 kHz kayıtta bu 180 000 kare × 8 bayt ≈ 1.4 MB'dir.

## Proje Yapısı

```
02-waveclip/
├── Cargo.toml
├── Cargo.lock
├── LICENSE.txt
├── README.md
├── .gitignore
├── src/
│   ├── lib.rs          çekirdek kütüphane ve modül bildirimleri
│   ├── main.rs         CLI kabuğu (clap)
│   ├── hata.rs         hata sınıfları + Display/Error uygulamaları
│   ├── wav.rs          RIFF ayrıştırıcı ve akış tabanlı PCM okuyucu
│   ├── energy.rs       RMS enerji zarfı, taban gürültü, konuşma bölgeleri
│   ├── rules.rs        dört hook kuralı, birleştirme, tekrar bastırma
│   ├── subtitle.rs     SRT ve WebVTT yazımı
│   └── rapor.rs        JSON şeması (Rapor, KirpmaPlani)
└── tests/
    ├── entegrasyon.rs  uçtan uca testler (28)
    └── yardimci/mod.rs geçici dizin + WAV üretici
```

Satır sayıları (satır sonu sayımı, boş satırlar dahil): `rules.rs` 916,
`wav.rs` 722, `tests/entegrasyon.rs` 649, `energy.rs` 563, `main.rs` 336,
`rapor.rs` 328, `subtitle.rs` 319, `tests/yardimci/mod.rs` 200, `hata.rs` 113,
`lib.rs` 47. Kaynak (`src/`) toplamı ≈ 2 344 satır, test (`tests/`) toplamı
≈ 849 satır, `README.md` 589 satır.

## Yapılandırma

Yapılandırma dosyası yoktur; tüm ayarlar komut satırı bayraklarıdır. Varsayılan
değerler `src/main.rs` ve `KuralAyar::default()` / `EnerjiAyar::default()`
içinde tanımlıdır.

### `scan` bayrakları

| Bayrak | Varsayılan | Etkisi |
|---|---|---|
| `<DOSYA>` | — | Okunacak WAV dosyası (konumsal) |
| `-o, --cikti` | standart çıktı | Raporun yazılacağı yol |
| `--pencere-ms` | `20` | RMS penceresi. Kare sayısı `hz × ms / 1000`; küçük değerler temporal çözünürlüğü artırır, dosya boyutunu değiştirmez |
| `--esik-db` | `10.0` | Konuşma eşiğinin taban gürültünün kaç dB üstü olacağı. Düşürülürse daha çok kare "konuşma" sayılır |
| `--en-alcak-db` | `-60.0` | Mutlak alt eşik (dBFS). Sayısallaştırma gürültüsünü ve çok sessiz kayıtları eler |
| `--ilk-acilis-sn` | `15.0` | "İlk açılış" kuralının aradığı pencere (saniye) |
| `--tepe-esik-db` | `6.0` | Tepenin kayıt ortalamasından en az bu kadar dB yüksek olması gerekir |
| `--sessizlik-sn` | `2.0` | "Uzun sessizlik sonrası giriş" için gereken en az boşluk (saniye) |
| `--pencere-sn` | `20.0` | "En yüksek enerji penceresi" kuralının sabit pencere genişliği (saniye) |
| `--maks-aday` | `30` | Döndürülecek en çok aday sayısı |

### `clips` bayrakları

| Bayrak | Varsayılan | Etkisi |
|---|---|---|
| `<DOSYA>` | — | Okunacak WAV dosyası |
| `-o, --cikti` | standart çıktı | Kırpma planının yazılacağı yol |
| `--kaydirma-sn` | `0.0` | Tüm adaylara uygulanan zaman kaydırması (saniye). Negatif değer mümkündür; sonuç `[0, sure_sn]` aralığına kırpılır |
| `--sira` | hepsi | Yalnızca bu 1 tabanlı sıra numaralı adaylar (virgülle ayrılmış, tekrarlanabilir) |
| `--pencere-ms` | `20` | Enerji penceresi (ms) |

### `srt` bayrakları

| Bayrak | Varsayılan | Etkisi |
|---|---|---|
| `<DOSYA>` | — | Okunacak WAV dosyası |
| `-o, --cikti` | standart çıktı | Altyazının yazılacağı yol |
| `-b, --bicim` | `srt` | `srt` veya `vtt`; başka değer hata verir |
| `--sira` | hepsi | Yalnızca bu 1 tabanlı sıra numaralı adaylar |
| `--maks-sure-sn` | `0.0` | Altyazı üretilen en uzun aday sınırı (saniye). `0` = sınır yok |

### Sabitler (derleme zamanı)

`wav::OKUMA_BLOK_BAYT = 65536`, `energy::ORNEK_TAMPON = 8192`,
`energy::VARSAYILAN_PENCERE_MS = 20`, `rules::TEPE_CEVRE_SN = 2.0`,
`Aday::TEKRAR_ESIGI_SN = 1.0`, `subtitle` minimum 1.0 sn / maksimum 7.0 sn /
80 ms boşluk / 42 karakter, `rapor::KURAL_SURESI = "waveclip-kural-1.0.0"`.

## Bilinen Sınırlamalar

Bu bölüm dürüst olmak zorundadır. Aşağıdakilerin hiçbiri "yolunda" değildir.

- **Sıkıştırılmış ses desteklenmiyor.** MP3, AAC, Opus, FLAC ve WAV içindeki
  ADPCM/doluluk biçimleri açık hata mesajıyla reddedilir. Podcast üreticisinin
  elindeki kayıt çoğunlukla MP3'tir; bu, ürün değerinin en belirgin biçimde
  daraldığı noktadır. Gerekçe: `MANIFEST.md` kart 02 "Sapma gerekçesi"
  (bağımlılık politikası ses kod çözücüsünü yasaklar).
- **Transkripsiyon yok.** Kelime düzeyinde zaman damgası üretilmediği için
  raporun `b07` kural tablosundaki "soru başlangıcı" (soru işareti + boşluk) ve
  "konuşmacı değişimi" (yön değişimi) kuralları **uygulanamaz**. Uygulanan dört
  kural raporun enerji/zaman analiziyle karşılanabilen kısmıdır; iki kural
  bilinçli olarak dışarıda bırakılmıştır.
- **Altyazı metni gerçek konuşma metni değildir.** Kelime damgası olmadığı için
  üretilen SRT/VTT girdileri zaman aralığını ve bölgenin tepe enerjisini taşıyan
  bir etiket metnidir (`[0:40] tepe -3.5 dBFS`). Gerçek altyazı metni için
  transkripsiyon katmanı gerekir.
- **Altyazı basma (burn-in) yok.** `libass` ve video kapsülleme kullanılmaz;
  MANIFEST kart 02 "Ertelenen" listesindedir. Yalnızca metin dosyası üretilir.
- **Dikey klip üretilmez.** `clips` bir **plan** üretir (JSON); görüntü
  kırpma/9:16 kadrajlama/yeniden kodlama yapılmaz. Gerçek video üretimi
  ertelenmiştir.
- **Kural ağırlıkları ölçülmemiştir.** Taban güven puanları (0.45–0.60) ve
  iki saniyelik tepe çevresi gibi sayılar raporun kendi uyarısıyla uyumlu
  olarak **gözle seçilmiş** başlangıç değerleridir; hiçbir ses kaydı üzerinde
  ölçülerek doğrulanmamıştır. Eşikler komut satırından değiştirilebilir, ancak
  "doğru" değerlerin ne olduğu bilinmemektedir.
- **DC ofseti çözülmez.** RMS, DC ofsetini de enerji olarak sayar. DC ofsetli
  bir kayıtta taban gürültü olduğundan daha yüksek görünür. Bilinen davranıştır
  ve testle sabitlenmiştir; DC çıkarma (high-pass filtre) uygulanmamıştır.
- **Konuşma algısı basittir.** Tek eşikli, tek geçişli bir ayrımdır. Müzik,
  gürültü ve nefes sesi "konuşma" sayılabilir; konuşma dili seçilmez. VAD
  (voice activity detection) veya makine öğrenmesi tabanlı algılama yoktur.
- **8-bit çözünürlük düşüktür.** 8-bit PCM'de tepe RMS ölçümü 16/24/32-bit
  sürümlere göre daha büyük sapma gösterir (testte ~0.2 dB tolerans).
- **Testler sentetik kayıtlarla yapılır.** Gerçek bir podcast kaydıyla
  doğrulama yapılmamıştır; kural motorunun gerçek konuşmada isabet oranı
  **ölçülmemiştir** ve bu belgede bir başarı oranı iddia edilmemektedir.
- **Windows dışı platformlar doğrulanmadı.** Geliştirme ve testler Windows
  üzerinde yapılmıştır. Yalnızca `std` kullanıldığı için başka platformlarda
  derlenmesi beklenir, ancak çalıştırılmamıştır.
- **`panic = "abort"` yalnızca release profilinde.** `cargo test` kendi
  profiliyle çalıştığı için entegrasyon testleri `should_panic` kullanamaz;
  bu depoda kullanılmamıştır.
- **`#[allow(..., reason = ...)]` kullanılmamıştır.** Clippy uyarılarının
  tamamı gerçek kod düzeltmeleriyle giderildi; bastırma etiketi yoktur.

## Gelecek Geliştirmeler

MANIFEST kart 02 "Ertelenen" listesinin tamamı:

- Gerçek dikey klip üretimi (9:16 kadrajlama, yüz bölgesine kaydırma).
- Altyazı basma (burn-in).
- Konuşmacı ayrımı.
- Kural seti düzenleyici arayüzü.
- Toplu video dışa aktarma.

Bunların dışında, doğal sonraki adımlar:

- DC ofset çıkarma (yüksek geçiren filtre) ve kanal başına RMS raporu.
- MP3/AAC için dış kod çözücü çağrısı veya isteğe bağlı `ffmpeg` yolu.
- Segment sınırlarını konuşma ritmine göre iyileştirme (kelime zamanlaması
  olmadan ulaşılabilecek en iyi sınır).
- Kural ağırlıklarının gerçek kayıtlar üzerinde kalibre edilmesi ve kalibrasyon
  dosyası (`config/kurallar.json`) desteği.
- WebVTT için stil/pozisyon (`NOTE` ve `STYLE` blokları) desteği.
- Çok dosyalı toplu tarama ve tek birleşik rapor.

## Troubleshooting

**Belirti:** `hata: dosya RIFF/WAVE degil (imza: "ID3"); yalnizca PCM iceren WAV okunur, MP3/AAC/Opus desteklenmez`
**Neden:** Girdi gerçekten MP3 (ya da AAC/Opus/FLAC) dosyası; program kapsam
dışı olduğu için başlığı okuyamaz.
**Çözüm:** Dosyayı PCM WAV'a dönüştürün (ör. `ffmpeg -i kayit.mp3 -ac 1 -ar
44100 -c:a pcm_s16le kayit.wav`) ve dönüştürülmüş dosyayı verin. Alternatif:
kaydı zaten WAV olarak alın.

**Belirti:** `hata: dosya sistemi hatasi: Sistem belirtilen dosyayı bulamıyor. (os error 2)`
**Neden:** Yol yanlış yazılmış ya da dosya taşınmış.
**Çözüm:** `waveclip info .\dosya.wav` ile yolu doğrulayın. Göreli yol yerine
tutin tam yol kullanın; PowerShell'de `.\` öneki unutulmamalıdır.

**Belirti:** `hata: 'data' kutusu yok` veya `hata: 'fmt ' kutusu yok veya 16 bayttan kisa`
**Neden:** Dosya WAV gibi görünüyor (`RIFF` + `WAVE` imzaları) ama ses kutusu
eksik ya da başlık bozuk. Bu, WAV içine `LIST`/başka kutular ekleyen bazı
düzenleyicilerin ürettiği bozuk dosyalarda görülür.
**Çözüm:** `waveclip info` çalıştırıp hangi kutunun eksik olduğunu görün. Dosya
onarılamıyorsa orijinal kaynaktan yeniden dışa aktarın.

**Belirti:** `hata: desteklenmeyen ses bicimi (wFormatTag = 0x11); yalnizca PCM (1) 8/16/24/32 bit desteklenir`
**Neden:** WAV kapsayıcısı içinde sıkıştırılmış bir ses kodu var (IMA ADPCM,
µ-law, A-law, GSM).
**Çözüm:** Dosyayı PCM'e dönüştürün. `waveclip info` `bicim` satırında hangi
`wFormatTag` değerinin okunduğunu gösterir.

**Belirti:** `hata: ayar hatasi: desteklenmeyen altyazi bicimi: ass (srt veya vtt)`
**Neden:** `-b` bayrağına yalnızca `srt` ve `vtt` kabul edilir.
**Çözüm:** `-b srt` ya da `-b vtt` kullanın.

**Belirti:** `hata: RIFF basligi cabuk: beklenen en az 12 bayt, dosyada 0 bayt var`
**Neden:** Dosya boş ya da yalnızca birkaç bayt.
**Çözüm:** `Get-Item .\dosya.wav` ile dosya boyutunu kontrol edin; 0 bayt ise
kayıt tamamlanmamıştır.

## Atıflar

- RIFF Resource Interchange File Format (RIFF) ve WAV — Microsoft,
  <https://learn.microsoft.com/windows/win32/xaudio2/resource-interchange-file-format>
- WAVE PCM biçimi — Microsoft Learn,
  <https://learn.microsoft.com/windows/win32/xaudio2/resource-interchange-file-format>
- `clap` (komut satırı ayrıştırma) — <https://docs.rs/clap/>
- `serde` / `serde_json` — <https://serde.rs/> · <https://docs.rs/serde_json/>
- Rust standart kütüphane belgeleri — <https://doc.rust-lang.org/std/>
- Rust 2021 edition rehberi — <https://doc.rust-lang.org/edition-guide/edition-2021/>
- WebVTT biçimi — W3C, <https://www.w3.org/TR/webvtt1/>
- SubRip (SRT) biçimi — Wikipedia,
  <https://en.wikipedia.org/wiki/SubRip>
- RMS seviyesi ve dBFS — Wikipedia,
  <https://en.wikipedia.org/wiki/Full-scale_amplitude>
- İç tasarımın kaynağı (yerel dosya, URL değildir):
  `%USERPROFILE%\Desktop\Fikirler\02-dalgakes-podcast-klip.html` — DalgaKes fikir
  raporu. Kural tablosu (b07), bellek bütçesi (b08) ve açık sorular (b16) bu
  dosyadan alınmıştır.
- Karar belgesi (yerel dosya): `%USERPROFILE%\Desktop\Projeler\MANIFEST.md` —
  Kart 02, MVP kapsamı ve sapma gerekçesi.
- Modellenen *düşünce* için whisper.cpp (raporun önerdiği, bu projede
  **kullanılmayan** yaklaşım) — <https://github.com/ggml-org/whisper.cpp>

Doğrudan kopyalanan üçüncü taraf kodu yoktur.

## Lisans

MIT lisansı. Tam metin: [`LICENSE.txt`](LICENSE.txt).

Telif: `Copyright (c) 2026 waveclip contributors`.
