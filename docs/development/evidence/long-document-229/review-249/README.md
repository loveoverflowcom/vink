# Review trước merge PR #249

Review ngày 16/11/2026: base và merge-base
`ca9369653a516715fea95ca71fbd818d54c15b0a`, HEAD được review
`233804451f2abb4a2b041c4406a549ed080b8153`. Measurement source vẫn là
`4f2883630ff27f36c7549e036e6b554166a86692`. Commit bổ sung chỉ lưu bằng chứng review;
không thay production hoặc test source, không tạo phép đo hiệu năng mới.

Không phát hiện regression có thể hành động do PR này đưa vào. Đã trace helper qua
`Vink::save_draft`, `PgStore::save_draft` và `load_draft`: baseline được khôi phục ngoài
timer, version tăng sau mỗi commit, readback kiểm toàn bộ document/title/version.
Các gap F1/F2/F3 trong [report](../../../long-document-budget.md#ba-finding-và-ranh-giới-tiếp-tục)
giữ nguyên phạm vi và owner; review này không giải quyết transport, loader hoặc Reader.

## Kiểm tra thực chạy

| Kiểm tra | Kết quả và giới hạn |
| --- | --- |
| [Đối soát độc lập](audit.json) | PASS 11 checksum lịch sử + 30 follow-up; 5 source fingerprints; 21 corpus; tính lại median/max/WAL, history và SSR từ raw JSON; 420 samples = 315 changed + 105 noop; 284 relative file links. Số scene browser là đối soát evidence cũ, không phải chạy browser mới. |
| [Regression PostgreSQL sau hoàn nguyên](pg-focused-restored.log) | PASS 4, benchmark opt-in ignored. Cluster riêng PostgreSQL 17.11, loopback port 55649; không dùng database của phiên khác. |
| [Mutation](mutation.json), [patch tạm](mutation.patch), [log trích](mutation-excerpt.log) | PASS về độ nhạy: chỉ giữ restore đầu tiên, bỏ các restore sau; test độc lập FAIL đúng assertion baseline ở dòng 131 của source tạm. Source gốc được khôi phục byte-identical, build lại và cả 4 checks PASS. Log trích chỉ bỏ hai giá trị DocumentData tổng hợp quá dài; hash log đầy đủ nằm trong JSON. |
| `cargo fmt --all -- --check`, diff/links/JSON/hash | PASS trên source cuối. Raw log giữ whitespace của công cụ. |

Build Cargo ban đầu FAIL trước khi chạy test vì linker hết đĩa (`errno=28`), xem
[build-initial.log](build-initial.log). Retry chỉ strip symbol của integration executable:
[build-stripped.log](build-stripped.log), [build-restored.log](build-restored.log).
Không dùng timing của binary này làm số benchmark. Target riêng được clone CoW từ cache
có sẵn; Cargo rebuild các package theo đường dẫn checkout review.

## Tái lập regression và mutation

Cwd lần review: `/Users/manhblue/projects/loveoverflow/vink-review-249`.
Log tạm và cluster riêng: `/tmp/vink-review-249-evidence`.

```sh
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1
# DATABASE_URL trỏ tới PostgreSQL local scratch có quyền CREATEDB.
cargo rustc --offline --locked -p vink-app --test postgres -- -C strip=symbols
# Tên executable là kết quả của lần build, không phải API ổn định:
target/debug/deps/postgres-05ea2833a56d8306 long_document_ --test-threads=1
```

Trong checkout sạch riêng, áp dụng `mutation.patch`, build bằng cùng lệnh, rồi chạy
`document_budget::long_document_samples_are_independent_durable_transitions --exact --test-threads=1`
trên executable vừa tạo. Kỳ vọng exit 101 tại baseline assertion. Hoàn nguyên patch, build
lại, chạy `long_document_` và kiểm hash source. Patch mutation chỉ là evidence, không được áp
dụng vào source đã commit.

Quick đầy đủ và probe 21 case/Chrome dùng lại
[evidence follow-up](../followup/README.md), sau khi kiểm source fingerprints và diff chỉ docs
từ measurement đến HEAD review. **NOT_RUN trong lượt review:** Quick/probe đầy đủ chạy mới,
full PostgreSQL/Web lanes, browser/device/IME, release/production và CI. Không suy kết quả
PostgreSQL tập trung thành full-lane PASS. `manifest.json` pin các file trong bundle review;
manifest/evidence lịch sử giữ nguyên.

Cluster review đã dừng; data scratch và target clone riêng đã được dọn sau kiểm tra.
Hash executable còn lưu ở [verification.json](verification.json); log gốc ở thư mục tạm.
