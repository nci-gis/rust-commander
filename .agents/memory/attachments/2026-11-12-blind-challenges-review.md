# xDev Commander v0.4 — Tổng hợp review và blind challenges

Ngày đánh giá: 12/11/2026. Nhánh: `feat/v0.4.0`. Mốc cố định: [`c4f51c417b28db903d50d1c6b7f2b71aa1731794`](https://github.com/nci-gis/rust-commander/commit/c4f51c417b28db903d50d1c6b7f2b71aa1731794).

## 1. Kết luận và phạm vi bằng chứng

Năm nhóm challenge người dùng đưa ra chạm đúng vùng kiến trúc, nhưng nhiều giải pháp đang đi trước việc xác định bottleneck. Đề xuất cho first stable: giữ kiến trúc Rust + ratatui hiện tại; củng cố tính đúng, trạng thái bất đồng bộ, giới hạn tài nguyên và khả năng phục hồi. Chưa có căn cứ để bắt buộc thêm arena, trie, mmap, Tokio hoặc một hệ transaction cho cả filesystem.

Trong lần kiểm tra này nhánh dev đã tiến từ `fd410e9` lên `c4f51c4`: F1 Commands, Config in use, dispatch theo context và tài liệu đã có code. Không tiếp tục dùng nhận xét cũ “F1 mới kickoff”. Đây không phải tuyên bố mọi tình huống của F1 đã được nghiệm thu bằng tay.

- Kết quả suite trên mốc mới: **706 passed, 3 ignored; chạy Linux với NO_COLOR được loại khỏi môi trường. Các test Windows-only không chạy trên Linux; không suy diễn đây là CI đa nền tảng cho head mới**.
- Mốc trước `fd410e9`: 662 tests passed, 3 ignored sau khi loại `NO_COLOR=1` khỏi môi trường test; lỗi ban đầu là test màu phụ thuộc môi trường.
- CI run [43181663296](https://github.com/nci-gis/rust-commander/actions/runs/43181663296) đã được xác minh xanh tại `31c139a`. Không suy diễn run này bao phủ code F1 vừa thêm.
- Benchmark trước: listing + sort 100.000 file khoảng 127 ms, executable tối ưu trên filesystem local của môi trường review. Các module listing/sort không đổi giữa hai mốc. Không phải latency toàn bộ pane, cold-cache benchmark hay số đo trên máy người dùng.
- Hai vấn đề compare và lỗi selection khi navigation chậm đã được tái hiện ở mốc trước; các đường code đó không đổi ở mốc mới. Không gọi đây là lần tái hiện mới trên toàn bộ mốc mới.
- Không có thay đổi code được giữ lại hay push. Báo cáo là review và đề xuất.

Ký hiệu bằng chứng: **R** = đã tái hiện; **C** = xác nhận bằng đọc code; **H** = giả thuyết cần test/fault injection; **D** = đối chiếu tài liệu chính thức. Một dòng có thể có nhiều loại bằng chứng.

Ưu tiên: **P0** = cần giải quyết hoặc vô hiệu hóa đường hành vi rủi ro trước RC; **P1** = củng cố/định nghĩa giới hạn trước stable; **P2** = tối ưu sau khi đo hoặc sau launch. Đây là ưu tiên đề xuất, không phải CVSS hay kết luận đã có mất dữ liệu thực tế.

## 2. Hiệu chỉnh các nhận định đầu vào

| Nhận định | Đánh giá | Cách hiểu chính xác hơn |
|---|---|---|
| Rust cho “an toàn bộ nhớ tuyệt đối” | Quá tuyệt đối | Safe Rust dựa vào tính sound của unsafe implementation mà nó sử dụng. Memory safety không bảo đảm không deadlock, không OOM, không mất dữ liệu, không chọn nhầm file. `forbid(unsafe_code)` trong repo không cấm unsafe bên trong dependency. |
| TUI Rust vượt trội hoàn toàn RAM/CPU so với Electron/Python/Node | Chưa có benchmark tương đương | Có lợi thế tiềm năng về footprint, nhưng cần cùng workload và tính cả terminal emulator, shell/child process khi so trải nghiệm toàn hệ thống. |
| One Commander là Electron | Sai ví dụ | Tài liệu chính thức OneCommander cho biết dùng WPF. Không lấy ví dụ này để chứng minh ưu thế so với Electron. |
| `Vec<PathBuf>` tự gây realloc/shifting nghiêm trọng lúc sort/filter | Sai quan hệ nguyên nhân | Di chuyển `PathBuf` không đồng nghĩa clone toàn bộ byte đường dẫn. Sort không mặc nhiên tăng capacity của Vec; stable sort có thể dùng scratch allocation. Repo thực tế giữ hai `Vec<Entry>` và clone ở refilter: đó mới là chi phí cần đo. |
| Trie cho tìm kiếm O(M) | Chỉ đúng với bài toán cụ thể | Tra prefix có chi phí theo query, nhưng trả K kết quả còn tốn O(K); xếp hạng cũng có chi phí. Prefix trie không trực tiếp thay thế fuzzy subsequence, substring hoặc glob đang dùng. |
| mmap dùng RAM gần bằng 0 và đọc không chặn | Sai | mmap thay đổi cách truy cập và copy; trang được đụng tới vẫn dùng bộ nhớ vật lý/page cache, có page fault và I/O. Mapping file bị sửa/truncate có hợp đồng an toàn khó đáp ứng. |
| Token bucket là debounce | Hai cơ chế khác nhau | Debounce gom theo khoảng yên lặng; token bucket giới hạn tốc độ/burst. Pure trailing debounce có thể trì hoãn vô hạn nếu sự kiện liên tục. Cần max-wait và cơ chế đọc bù khi mất event. |
| WAL cho rollback nguyên tử cả batch và resume đúng byte | Chưa đủ điều kiện | Cần protocol durability, commit boundary, dữ liệu trước ghi đè, idempotence, kiểm tra identity, xử lý tác nhân bên ngoài. Rename một file, phục hồi process và transaction cả batch là ba mức bảo đảm khác nhau. |

## 3. Bảng kỹ thuật tổng hợp

| ID / trụ cột | Challenge và hiện trạng repo | Bằng chứng / ảnh hưởng | Giải pháp đơn giản được khuyến nghị | Cách kiểm chứng / tiêu chí | Quyết định |
|---|---|---|---|---|---|
| A01 — dữ liệu | Hai `Vec<Entry>` (`all`, `entries`); refilter clone tên/path; membership marked quét visible lồng nhau | C: O(M×V) ở phần giữ marks; nhiều allocation; chưa có số đo end-to-end cho lựa chọn lớn | Sửa membership bằng HashSet trước. Nếu clone/RAM vẫn đáng kể, một snapshot sở hữu entry + `Vec<usize>` làm view; không cần arena custom | 10k/100k entry; marks 0/10/10k; đo allocation, peak RSS, p95 filter; selection đúng qua reload | P1 phần membership; P2 chuyển mô hình theo số đo |
| A02 — tìm kiếm | Fuzzy/subsequence, glob và sort là các semantics khác nhau; F1 mới có `fuzzy::Query` chuẩn bị query | C/D: trie prefix không đáp ứng cùng tập kết quả; không thể gọi O(M) cho toàn bộ fuzzy search | Giữ semantics; chuẩn bị query một lần, cache có kiểm soát; chỉ thêm prefix index nếu prefix thật sự là workload lớn | Unicode, smart case, query dài; đối chiếu tập kết quả và ranking; không chỉ đo tốc độ | P2, không thêm trie cho v0.4 chỉ vì lý thuyết |
| A03 — viewer | `Document` giữ file handle + line offsets, đọc vùng cần xem; scan 256 KiB; cap 4m dòng và 64 KiB/dòng; quick view từ chối >64 MiB | C: giả thuyết read_to_string toàn file không đúng. Nhưng index text thực hiện trước khi open trả về; F3 gọi đồng bộ | Progressive/background indexing có cancel; sparse checkpoints nếu cần giảm index; giới hạn rõ cho long line và file đang đổi | File lớn ít newline, nhiều dòng ngắn, log đang append/truncate/rotate; first paint, Esc, RSS và thông báo phần chưa index | P1 test/giới hạn; triển khai incremental theo mục tiêu release |
| A04 — mmap | Repo chưa cần mmap để tránh đọc toàn file vào RAM | D: memmap2 file-backed map là unsafe; file thay đổi có thể gây UB theo hợp đồng crate; truncate có thể gây lỗi hệ thống | Không thêm vào v0.4. Nếu thử sau này: chỉ trên nguồn bất biến/snapshot do app kiểm soát; đối chiếu buffered I/O | Cùng workload/cache; đo minor/major faults, RSS, first paint, thay đổi file; chứng minh invariant trước lợi ích | P2, deferred |
| A05 — rendering | Vòng vẽ row chỉ chạy `0..rows.height`; ratatui diff buffer; Quiet giảm output khi không đổi | C/D: viewport virtualization đã có. Tuy nhiên status summary và đếm entry còn quét toàn danh sách ở đường draw | Profile toàn pipeline; cache aggregate theo generation/selection-change, không cache mù. Không xây ring buffer mới trước khi đo | Cùng viewport, 1k/100k entry; marks on/off; tách thời gian model, format, diff và terminal write | P1 đo; tối ưu có mục tiêu |
| A06 — listing background | Có worker đọc, UI đợi tối đa 150 ms, giữ pane cũ, Esc bỏ chờ; sort/refilter vẫn ở UI | C: bớt treo không có nghĩa hoàn toàn nonblocking; hủy chờ không dừng syscall | Bounded worker/queue; generation; coalesce request. Nếu streaming: hiển thị provisional rõ, pin selection, kết quả cũ không được apply | Mount chậm, chuyển A→B→C nhanh, hủy lặp; old result không thay current view; thread/queue không tăng vô hạn | P1 |
| A07 — watcher | notify dùng channel không giới hạn; changed gom path vào HashSet mỗi lần drain; to_reload là tập pane; có polling fallback | C: đã coalesce, chưa có cửa sổ debounce/max-wait rõ; drain không có budget. Event storm/queue growth là H | Dirty flag theo directory, giới hạn event processing mỗi tick, debounce + max-wait; overflow/error đặt trạng thái cần rescan; không dùng event như nguồn sự thật | Build tree 10k create/rename/delete, event liên tục, lỗi watcher; đo input p95, queue, số rescan; cuối cùng khớp disk | P1 |
| A08 — copy/recovery | Có temp-then-rename, journal, resume theo cơ chế hiện có; không phải tất cả file trong batch cùng commit | C/D: atomic visibility khác durability; flush không phải sync xuống đĩa; không có bảo đảm rollback batch | Định nghĩa bảo đảm theo từng operation và loại sự cố; nếu cần durability, thiết kế thứ tự sync journal/file/directory theo OS/FS. Giữ completed files, báo phần còn lại | Kill process tại các phase; thử storage fault ở VM riêng; so manifest/hash nguồn/đích; kiểm tra rerun recovery hai lần | P0 đóng hợp đồng/bug dữ liệu; không xây full WAL transaction trước launch |
| B01 — compare | `read_dir` lỗi thành rỗng; một số lỗi metadata bị bỏ; tên đi qua lossy String rồi dựng path | R/C: missing directory bị hiểu LeftOnly; non-UTF-8 sinh source path không tồn tại | Dùng PathBuf/OsString; Missing/Unreadable/Incomplete; không tự lập sync plan cho vùng chưa xác định | Permission denied, ENOENT, file biến mất giữa scan, hai tên không UTF-8 khác nhau; lỗi phải hiện rõ | P0 |
| B02 — async selection | Enter từ results gọi set_cwd rồi dựng target từ cwd cũ khi navigation chưa xong | R: thư mục mở đúng nhưng cursor ở `..` | Selection intent gắn request và được resolve tại destination sau completion; pin file identity/path phù hợp | Results→slow directory→F3; symlink; canceled/replaced request; selected target luôn đúng | P0 sửa regression nhỏ trước RC |
| B03 — panelize | Parser nhận diện đường dẫn trong văn bản, không phải transport lossless | R/C: tên toàn số bị bỏ; `report:2026.txt` bị hiểu file+line; tên có space không quote bị tách | Tách heuristic text khỏi exact-path import; preview tập kết quả; exact mode nên dùng NUL hoặc schema xác định | Spaces, colon, quotes, newline, raw Unix names; đối chiếu input/output và đối tượng F5/F8 | P1 giới hạn/UX; exact mode theo phạm vi được duyệt |
| B04 — journal failure | `Journal::line` bỏ qua lỗi write/flush; batch rename chuyển lỗi tạo journal thành None | C: mất journal sau khi bật safe có thể không được báo đúng; mất recovery sau ENOSPC là H cần fault injection | Trả Result cho ghi intent; trước thao tác không đảo ngược, dừng hoặc chuyển trạng thái thiếu bảo đảm và yêu cầu quyết định rõ. Không tự chạy tiếp như có log | ENOSPC/EIO khi create/write log và giữa batch rename; không báo safe/recoverable khi log đã hỏng | P0 xác minh và xử lý trước stable |
| B05 — identity/TOCTOU | Tên/path không khóa một đối tượng; preview/confirm và execute ở hai thời điểm; có bảo vệ no-replace một số đường | H, chưa kết luận có exploit hay mất dữ liệu | Dùng handle/identity khi phù hợp; revalidate plan; no-replace cho tạo mới; khi đích đổi sau confirm phải xử lý conflict. Metadata/hash không thay thế snapshot tuyệt đối | Tiến trình khác replace file/symlink sau preview; source bị sửa khi copy; không đụng target ngoài ý định | P1 fault tests, P0 nếu tái hiện sai đối tượng |
| B06 — metadata & filesystem | copy_bytes giữ permissions/mtime theo best effort và bỏ qua một số lỗi; file data không bao hàm mọi metadata | C/H: ACL/xattr/ADS/sparse/hardlink semantics cần xác định theo platform, chưa đo fidelity toàn bộ | Công bố mức preserve; báo partial success nếu thuộc cam kết mà thất bại; không quảng bá như backup đầy đủ | Executable, readonly, ACL/xattr/ADS theo OS, sparse và hardlink; move khác filesystem; source chỉ xóa khi đạt contract | P1 hợp đồng + test |
| B07 — Unicode | Sort có fold riêng nhiều script; fuzzy fold có quy tắc riêng; raw identity khác display equivalence | C/H: “nhìn giống nhau” không có nghĩa cùng file; không nên dùng sort key làm identity | Tách identity, display, search và collation key. Không gọi custom fold là đầy đủ Unicode Collation Algorithm nếu chưa chứng minh | NFC/NFD, case-sensitive/insensitive FS, full-width, đ/d, combining marks; total-order property và no collision trong thao tác | P1 |
| B08 — tài nguyên toàn app | Giới hạn 10 lists ×10k rows tốt nhưng không tự giới hạn channel, find đang chạy, terminal history, index viewer và clone | C/H: nhiều tính năng cùng chạy có thể vượt ngân sách dù từng tính năng ổn | Resource budget chung; bounded work; ưu tiên input/navigation; cache eviction có generation; coalesce progress nhưng giữ completion/error | Find + copy + viewer + noisy shell cùng lúc; RSS/thread/FD/queue plateau; Esc/F10 đáp ứng | P1 |
| B09 — release proof | Size test skip khi thiếu release binary; CI test trước build; macOS còn bước lipo | C: green CI không chứng minh limit của artifact phát hành | Check sau build/package, thiếu artifact fail, từng target; chốt 7 MiB theo slice hay universal | Cố tình vượt size phải làm job fail; tải đúng archive và smoke từ máy sạch | P0 gate |
| B10 — end-user trust | F1 đã triển khai; safety/undo/recovery là hành vi theo context; không chỉ số lượng test | C/H: command palette chạy action phải giữ đúng origin context, safe mode và viewer nằm dưới | Test workflow xuyên tính năng; giữ tất cả entry point qua cùng policy guard; ít bước nhưng hậu quả rõ | F1 từ pane/shell/viewer, read-only/root, pending job; user biết nguồn/đích và phần đã/không hoàn tất | P1 và điều kiện nghiệm thu |

## 4. Ba mâu thuẫn cần giải bằng thiết kế

### Phản hồi sớm ↔ thứ tự và đối tượng thao tác ổn định

Streaming first paint cho kết quả sớm nhưng khi sort lại từng batch, vị trí row đổi. Tách theo thời gian: pending → kết quả tạm → snapshot đã chấp nhận; giữ selection theo entry identity trong generation hiện tại. Một phím F8 không được đổi đối tượng vì row vừa dịch chuyển. Đây là ứng dụng phân chia và tách theo thời gian; không chỉ là chọn thread hay async runtime.

### Ít bộ nhớ ↔ nhiều cách nhìn và tìm kiếm nhanh

Giữ một kho entry, tạo view bằng indices; metadata đắt tiền tải theo nhu cầu. Cache key/query hữu ích có giới hạn và invalidation; arena chỉ đáng cân nhắc khi allocator thực sự là bottleneck. Tránh biến tối ưu RAM thành lifetime/identity phức tạp hơn dữ liệu gốc.

### Throughput cao ↔ phục hồi và durability mạnh

Thêm sync cho mọi bước làm chậm; bỏ sync làm lời hứa sau mất điện yếu hơn. Tách mức bảo đảm và phase commit, chọn theo nhu cầu/rủi ro. Với v0.4, ưu tiên contract chính xác và fault tests cho cơ chế hiện có. Không gọi best-effort recovery là atomic transaction.

## 5. Bộ kiểm chứng đề xuất

Các ngân sách dưới đây là đề xuất để chốt trên máy chuẩn, không phải số đo đã đạt.

| Nhóm test | Dữ liệu/kịch bản | Đo gì | Điều kiện đạt |
|---|---|---|---|
| Directory | 1k/10k/100k names; ASCII/Unicode; marks 0/10/10k | First paint, ready-to-act, p50/p95/p99 input, peak RSS, allocations | Tách budget core scan/sort khỏi full pane. Mục tiêu input p95 ≤50 ms, không khoảng block ứng dụng >150 ms trên máy chuẩn |
| Event storm | 10k changes rồi dừng; stream liên tục; drop/overflow notification | Queue/threads, rescans/s, cuối cùng hội tụ về disk | Không backlog vô hạn; có max-wait; sau khi yên thì rescan đúng |
| Viewer | 1–2 GiB text; long line; 4m+ dòng; growing/truncated/rotated log | First paint, cancellation, index bytes, I/O | Không cần full scan trước phản hồi nếu cam kết nhanh; giới hạn phải hiển thị; lỗi không giả làm nội dung rỗng |
| Selection | Navigate chậm; resort/filter/refresh giữa thao tác; back/forward | Đối tượng thực tế nhận action | Không thao tác nhầm file; stale completion bị bỏ |
| Recovery | Kill sau intent, trong copy, trước/sau rename, trước xóa nguồn | Manifest/hash, orphan files, idempotence | Không xóa source chưa đạt copy contract; recovery không phá dữ liệu mới của tiến trình khác |
| Storage failure | ENOSPC/EIO, permission changes; VM power-off ở nhiều phase | Trạng thái user thấy và dữ liệu trên đĩa | Báo đúng phần chưa hoàn tất; chỉ cam kết durability trong phạm vi đã kiểm chứng. Kill -9 không thay thế power-loss test |
| Cross-platform | Windows cmd/pwsh; macOS Terminal/iTerm; Linux/tmux/SSH | Cùng workflow đầu-cuối; phím, path, clipboard, metadata | Chính artifact release chạy được; kết quả và hạn chế được ghi lại |

Benchmark phải ghi OS, filesystem, thiết bị, build profile, cache warm/cold, số lần chạy và workload. Không lấy một lần 127 ms để kết luận vượt mọi đối thủ. CI smoke/performance guard dùng ngưỡng phù hợp độ nhiễu runner; benchmark phát hành dùng môi trường chuẩn.

## 6. Thứ tự thực hiện đề xuất

1. **Đóng correctness/recovery gaps:** B01, B02, B04; đưa lỗi ghi journal và trạng thái thiếu bảo đảm lên rõ ràng; regression tests trước sửa.
2. **Đóng release gates:** B09, compatibility promise và phạm vi crash recovery; không thiếu artifact rồi vẫn pass.
3. **Đo một vertical slice:** search/panelize → chọn nhiều → preview → copy/move → refresh → undo. Đo UI latency, RAM và độ đúng đối tượng cùng lúc.
4. **Tối ưu nhỏ trước:** membership HashSet, prepared query, cached aggregate theo generation, giới hạn worker/event processing. Mọi cache mới phải có invalidation test.
5. **Cân nhắc index view sau số đo.** Tránh refactor lớn ngay trước RC nếu đường hiện tại đạt budget và không có bug liên quan.
6. **RC fixes-only:** matrix người dùng, fault tests, test artifact. Những cơ chế mới lớn (mmap, trie, arena custom, full WAL transaction) để sau launch nếu có bằng chứng cần.

## 7. Nguồn code và tài liệu

### Code ở đúng snapshot

- [FilePane: state/filter/render/navigation](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd/src/pane/files.rs)
- [Document: indexing và đọc theo vùng](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd-core/src/document.rs)
- [Quick view](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd/src/app/pictures.rs)
- [Watcher](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd/src/watch.rs), [event loop](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd/src/app/run.rs)
- [Compare](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd-core/src/compare.rs), [ops](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd-core/src/ops.rs)
- [Journal](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd-core/src/journal.rs), [batch rename](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd-core/src/batch_rename.rs)
- [Found-path parser](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd-core/src/found.rs), [fuzzy](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd-core/src/fuzzy.rs), [sort](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd-core/src/sort.rs)
- [Commands](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd/src/commands.rs), [budgets](https://github.com/nci-gis/rust-commander/blob/c4f51c4/crates/xdevcmd/tests/budgets.rs), [CI](https://github.com/nci-gis/rust-commander/blob/c4f51c4/.github/workflows/ci.yml)

### Primary sources đối chiếu kỹ thuật

- [Rustonomicon — Safe/Unsafe contracts](https://doc.rust-lang.org/nomicon/safe-unsafe-meaning.html)
- [Rust Vec documentation](https://doc.rust-lang.org/std/vec/struct.Vec.html)
- [memmap2 MmapOptions — Safety](https://docs.rs/memmap2/latest/memmap2/struct.MmapOptions.html)
- [Rust File — sync_all/sync_data/flush](https://doc.rust-lang.org/std/fs/struct.File.html)
- [Tokio filesystem implementation](https://docs.rs/tokio/latest/tokio/fs/), [spawn_blocking và cancellation](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
- [Ratatui rendering](https://www.ratatui.rs/concepts/rendering/), [Terminal rendering pipeline](https://docs.rs/ratatui-core/latest/ratatui_core/terminal/struct.Terminal.html)
- [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html): tham khảo điều kiện durability/protocol, không suy diễn một file manager có transaction tương đương SQLite.
- [OneCommander system requirements — WPF](https://onecommander.com/help2/Systemrequirements.html)
