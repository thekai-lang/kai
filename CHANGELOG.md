# Changelog

Semua perubahan penting pada proyek Kai akan didokumentasikan dalam file ini.
Proyek ini mengikuti [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.11] - 2026-08-29
### Added
- **kai-sync**: Dukungan penuh pengunduhan spesifikasi OpenAPI YAML/JSON.
- **kai-sync**: Resolusi `$ref` bawaan untuk komponen modular (misal `#/components/schemas/X`).
- **kai-sync**: Ekstraksi parameter OpenAPI (*query*, *path*, *header*, *body*) dan tipe bersarang (*nested object & arrays*).
- **kai-parser**: Menambahkan pengenalan blok DSL dengan *keyword* `with` (`with query:`, `with body:`, `with auth:`).
- **kai-typecheck**: Evaluasi ketat secara statis antara parameter kode dengan *snapshot* API, mencegah `type mismatch` dan luputnya pengisian parameter `required`.

### Fixed
- **kai-ownership**: Memperbaiki kebocoran memori (44 byte) pada nilai *temporary* yang di-*inline* di dalam ekspresi `return` (terdeteksi via LeakSanitizer/ASan).
- **kai-typecheck**: Membersihkan seluruh *warning* `clippy` di seluruh kode tipe (*100% clean*).
- Menghapus dependensi C (`openssl`) dari `kai-sync` dengan berpindah ke `rustls`, memungkinkan *ASan test suite* dapat berjalan bebas pada *pipeline*.
