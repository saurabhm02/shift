# Blueprint: shift — a Rust CLI for local file search and Instagram saved-post search, built as a Rust learning project

Revision 2 (2026-10-13): the file-search half now follows fsearch's design and command behavior (daemon-first thin client, bare-query search, lock file, launchd install and uninstall, base plus overlay index, watch-before-scan, idempotent directory diff, trigram index with verification). Storage and system access are written in safe Rust. The Instagram half is unchanged.

## Context

The author is learning Rust from scratch and wants to learn it by building one real tool end to end, not by reading chapters. The tool is called **shift** (crate name `shift`, binary name `shift`) and lives in its own new project folder, `~/Developer/gyaan/shift/`. It is a separate project from Chotu.

Two search problems exist today and neither is solved:

1. **Local files.** macOS Spotlight is slow, opaque and weak at content search. The reference for what the author wants is the open-source tool fsearch (github.com/noahdunnagan/fsearch, Rust, macOS, MIT, about 9,800 lines). The author read its source and wants shift's file search to follow its design and behavior. fsearch works as follows:
   - A background daemon owns two indexes: a name index of every entry on disk, and a trigram content index of text files.
   - The name index is one flat structure written one directory block at a time in depth-first order, so a directory's whole subtree is a single contiguous range and scoping a search to a folder is a range bound. Names are interned so each distinct name is stored once and scored once.
   - Name matching is fuzzy (subsequence scoring with boundary, camelCase and consecutive-run bonuses) and tolerates one typo in words of five or more letters.
   - Content search turns a query into trigrams, intersects posting lists to pick candidate files, then reads those candidate files fresh from disk to verify real matches.
   - The index stays current from file-system events. Every directory event is handled by listing that one directory and diffing it against the index, which is idempotent, so replays, duplicates and races are harmless. The live state is an immutable base, a set of removed base entries, and a small overlay of additions, folded back into a new base by compaction.
   - Events are watched before the first scan so nothing changed during the scan is missed.
   - Exactly one process owns the index, enforced by a lock file the operating system releases when the process dies.
   - The command line is a thin client of the daemon over a Unix socket speaking JSON lines. A bare query is a search. The daemon starts on first use. Install copies the binary to `~/.local/bin` and optionally registers a macOS login agent that runs the daemon at login and restarts it if it dies. Uninstall removes the agent and keeps the index.
   The author wants to build their own version from scratch in their own words, not copy its code.
2. **Instagram saved posts.** The author has 2,493 saved posts (2,141 reels, 352 regular posts) and cannot search them by what they say. They downloaded their data export from Instagram, located at `~/Downloads/Instagram Account Settings Oct 11 2026/`. Inside it, `your_instagram_activity/saved/saved_posts.html` (about 6 MB, HTML not JSON) holds one entry per post with URL, caption, hashtags and owner username; 2,470 of the 2,493 have a caption. `saved_collections.html` (about 33 KB) holds named collections (for example one called "money") with their posts. The export contains no video or audio. An earlier plan for this part exists at `docs/saved-search-plan.md` in the Chotu project and is superseded by this blueprint where they differ.

Tooling already installed on the author's Mac: `cargo` and `rustc` (stable), `yt-dlp`, `ffmpeg`, `ollama`, `whisper-cli` (whisper.cpp), `rg`, `fd`. The author's global rules require: free local models only, Bun as JavaScript package manager (not relevant to this Rust project), and git is read-only for the assistant, so all version-control actions are done by the author.

## Problem

The author has no single, self-built tool that (a) finds local files instantly by fuzzy name and by content in the way fsearch does, (b) finds saved Instagram posts by meaning and spoken content, and (c) teaches Rust properly while being built. Existing tools solve pieces, but using them teaches nothing, and the Instagram saves are unsearchable by content today.

The core problem is therefore twofold: deliver a working combined search CLI with a daemon that behaves like fsearch, and deliver it through a build order in which every phase introduces a small, named set of Rust concepts, produces something that runs and is tested, and only depends on concepts already learned. The scope is large for a first Rust project (thirteen phases). The build order must make the project useful long before the last phase so the author is never stuck with a half-built system. fsearch's lowest-level techniques (memory-mapped blobs, raw system calls, unsafe code) are deliberately left out so the architecture can be learned without that burden.

## Requirements

### A. Project foundation

1. The project is a single Cargo package named `shift` that builds a single binary named `shift`, located at `~/Developer/gyaan/shift/`. `cargo build` completes with zero warnings. The project contains no `unsafe` code.
2. `shift --help` lists exactly these top-level commands: `insta`, `files`, `serve`, `stop`, `status`, `stdio`, `bench`, `install`, `uninstall`, `config`. Any first argument that is not one of these is treated as a file-search query, so `shift rust notes` searches files. `shift <command> --help` documents every subcommand and flag.
3. All persistent data lives under one root directory, by default `~/Library/Application Support/Shift/`. It contains the configuration file `config.toml`, the SQLite database, the name index file, the content segment folder, `daemon.lock`, the daemon socket and `daemon.log`. If the environment variable `SHIFT_HOME` is set, that directory is the root instead, so tests never touch real user data.
4. Configuration is read from the TOML file. If it does not exist, built-in defaults are used and no file is created implicitly. `shift config show` prints the effective configuration, and `shift config path` prints the file location.
5. Every command that prints results supports `--json`, which prints one JSON object per line and nothing else on stdout. Human output goes to stdout, diagnostics go to stderr, and exit status is 0 on success, 1 on a runtime error, and 2 on a usage error.

### B. Instagram import

6. `shift insta import <export_dir>` reads `your_instagram_activity/saved/saved_posts.html` and `saved_collections.html` under `<export_dir>` and stores every post with: shortcode, URL, kind (reel or post, from the URL path), owner username, caption, hashtags, and collection name (empty if none).
7. Running the import on the author's export stores exactly 2,493 posts, with 2,141 of kind reel and 352 of kind post, and completes in under 10 seconds.
8. Running the import twice stores the same 2,493 posts and creates no duplicates. A post already present has its caption, hashtags and collection updated, and its pipeline status is preserved.
9. Import prints a summary: posts read, posts new, posts updated, posts without caption. If the HTML file is missing or has zero recognizable entries, the command exits with status 1 and a message naming the missing file or the zero-entry condition, and the database is unchanged.

### C. Embeddings (shared by Instagram and files)

10. Embeddings are produced only by a local Ollama server using the model `bge-m3` (1024 dimensions). The Ollama address and model name are configurable, with defaults `http://localhost:11434` and `bge-m3`. No remote or paid service is ever called.
11. `shift insta embed` embeds every Instagram text chunk that has no vector yet, in batches, and can be interrupted and re-run without redoing finished chunks. After a full run on the author's data, every one of the 2,470 non-empty captions has exactly one caption chunk with a vector.
12. If Ollama is not reachable or the model is not installed, embedding commands exit with status 1 and a message that states which of the two failed and the exact command to fix it. No partial garbage is written.
13. A vector is stored as little-endian 32-bit floats in a BLOB and reads back to values identical to what was written.

### D. Instagram search

14. `shift insta search "<text>"` runs in the calling process against the SQLite database (it does not use the daemon) and returns the top 10 chunks by default, merged into one list per post (a post appears once, with its best chunk), ranked by reciprocal rank fusion of two rankings: cosine similarity over vectors, and BM25 over a full-text index of chunk text. The constant is 60.
15. Each result shows: rank, fused score, kind, owner, collection if any, the matched text snippet, the timestamp range for transcript chunks, and the post URL.
16. `shift insta search` supports `--collection <name>`, `--owner <name>`, `--kind reel|post` and `--top <n>` (1 to 100). Filters are applied before ranking, so `--top` always returns up to n results that satisfy the filters.
17. Searching 30,000 chunks returns in under 150 ms at the median on the author's machine, excluding the one-time Ollama call that embeds the query.
18. A query containing only text that matches nothing returns zero results with exit status 0 and a one-line "no matches" message, not an error.

### E. Instagram audio and transcripts

19. `shift insta fetch --limit <n>` (default 50) downloads audio only, using `yt-dlp` with the browser cookie option, for the next n posts of kind reel whose status is `imported`. It waits a random 5 to 15 seconds between downloads, stops immediately on a rate-limit or login-challenge error, and never downloads the same post twice.
20. A post that fails to download (deleted, private, blocked) is marked `failed` with the error text, is skipped by later `fetch` runs unless `--retry-failed` is passed, and remains searchable by caption.
21. `shift insta transcribe` runs `whisper-cli` on every post whose status is `audio_ready`, using the model path from configuration, and stores timestamped transcript chunks of 30 to 60 seconds each, split only at segment boundaries. Chunks are then embedded by `shift insta embed` and become searchable.
22. Each post has exactly one status at any time, from this set: `imported`, `audio_ready`, `transcribed`, `embedded`, `failed`. `shift insta stats` prints the count per status and the number of failed posts with their reasons grouped.
23. Killing any of `fetch`, `transcribe` or `embed` at any moment and re-running it completes the remaining work without redoing finished posts and without corrupting the database.

### F. File name search

24. `shift <query>` searches the name index through the daemon and starts the daemon in the background if it is not running. Results print one path per line with size and modified time in human mode. While the very first index is still being built, the command exits with status 1 and prints how many entries have been listed so far, instead of waiting or printing partial results.
25. Name matching is fuzzy: the characters of each query term must appear in order in the file name, scored higher for consecutive characters, word-boundary starts, camelCase humps and prefix matches, and lower for gaps. Terms in one query are combined with AND. A term of 5 or more letters also matches names within one edit (insert, delete, substitute) of any name word. Ordering is score descending, then path length ascending, then path alphabetical, so output is deterministic.
26. The query language supports these filters: `ext:<ext>`, `type:file|dir`, `in:<folder>`, `size:<op><n><unit>` with operators `<` and `>` and units `k`, `m`, `g` (1024-based), `mtime:<op><n><unit>` with units `m`, `h`, `d` (`<7d` means modified within 7 days, `>30d` means older than 30 days), `path:<substring>`, and `limit:<n>`. It supports these operators on a name term: a leading single quote for exact substring, a leading `^` for prefix, a trailing `$` for suffix, and a leading `!` to exclude. Unknown filters exit with status 2 and name the bad filter. Searches scoped with `in:` examine only the entries inside that folder's contiguous range of the index.
27. Names indexed exclude, by default and configurable, these directory names: `.git`, `node_modules`, `target`, `Library/Caches`, `Library/Mobile Documents`, `Library/CloudStorage`, `.Trash`. The daemon detects at startup whether Full Disk Access has been granted to the binary. If it has not, it skips the consent-gated folders under the home directory and logs that it did. Directories that cannot be read are skipped and counted. `shift status` reports both the Full Disk Access state and the skipped-directory count.
28. On a synthetic tree of 200,000 files, a name query answered by a warm daemon returns in under 20 ms at the 95th percentile, and a full index build completes in under 60 seconds.

### G. File content search

29. `grep:<text>` in a query searches inside files that pass the other filters. Matching lines are printed as path, line number and the line text. Content search is smart-case: an all-lowercase pattern ignores case, a pattern with any uppercase letter is case-sensitive.
30. `regex:<pattern>` (alias `re:`) searches file contents with a regular expression and obeys the same smart-case rule. An invalid pattern exits with status 2 and prints the parse error.
31. Only text files are content-indexed: files of size at most 1 MiB whose first 8 KiB contain no NUL byte. Binary and oversized files are skipped and counted.
32. A trigram index maps each case-folded 3-byte sequence to the files containing it, stored as delta-encoded sorted lists in immutable segment files. A `grep:` literal of 3 or more bytes uses the index to select candidate files and then verifies matches by reading those files from disk, so results never come from stale index text. A literal shorter than 3 bytes is rejected with status 2 and a message stating the minimum.
33. `regex:` uses the index when a literal of 3 or more bytes can be extracted from the pattern, and otherwise scans all indexed text files and prints a note that the scan was unindexed.
34. On a synthetic tree of 20,000 text files, an indexed `grep:` of a literal that appears in fewer than 50 files returns in under 100 ms at the 95th percentile with a warm daemon, and returns exactly the same set of lines as `rg --no-ignore --hidden -F -i` over the same tree.

### H. Semantic search over local text files

35. `shift files semantic add <dir>` registers a directory for semantic indexing. Only registered directories are ever embedded; the whole disk is never embedded. `semantic list` and `semantic remove <dir>` exist.
36. Files in registered directories with extensions `md`, `txt`, `rs`, `py`, `ts`, `js`, `go`, `toml`, `json`, `yaml`, `yml`, `rst`, `org` and size at most 512 KiB are split into chunks of about 1,200 characters with 150 characters of overlap, split at line boundaries, each stored with file path and starting line, and embedded with the same embedding module and model as Instagram.
37. `shift files semantic search "<text>"` goes through the daemon and returns the top 10 chunks, one per file, ranked by the same fusion method as requirement 14, showing path, starting line and snippet.
38. A file whose size and modified time are unchanged is not re-embedded. A changed file has its old chunks replaced. A deleted file has its chunks removed. The total chunk count is capped at a configurable limit (default 100,000); reaching it stops indexing with a clear message and exit status 1.

### I. Daemon and install

39. `shift serve` runs the daemon in the foreground. The daemon owns the name index, content index and embedding queue, listens on a Unix socket in the root directory, and writes `daemon.log`. `shift stop` shuts it down cleanly within 5 seconds and removes the socket. `shift status` prints: ready or still indexing, entry count, directory count, overlay count, removed count, content document count, segment count, pending content work, Full Disk Access state, skipped-directory count, and whether this daemon owns the index.
40. At most one daemon runs per root, enforced by an exclusive advisory lock on `daemon.lock` held for the life of the process. Starting a second `serve` prints that it is already running and exits with status 0. A daemon killed with a hard kill releases the lock automatically, and a leftover socket file with no process behind it is detected and replaced on the next start.
41. File-name, content and semantic queries are always answered by the daemon; there is no in-process fallback for them. `shift bench <query>` is the only in-process path: it loads the saved index file, runs the query 20 times with an unrelated query between runs, and prints the first, median and minimum times to stderr and the first 10 results to stdout. For the same query and the same index state, `bench` and the daemon return the same results in the same order.
42. The socket speaks one JSON request per line and answers with one JSON object per line, with a final line marking the end of results, usable from any language. `shift stdio` forwards its standard input lines to the daemon and prints the daemon's lines on standard output until input ends. The protocol is documented in the project README, including request kinds and error responses.
43. The daemon watches indexed roots with the operating system's file-event facility, and starts watching before the first scan begins. Every event is handled by listing the one affected directory and diffing it against the index (add what is new, mark removed what is gone, update what changed), and applying the same event twice changes nothing. A file created, renamed, modified or deleted under an indexed root appears in (or disappears from) name and content search results within 2 seconds, without a manual re-index. Bursts of events are batched.
44. The live name index is an immutable base plus a set of removed base entries plus an overlay of additions. The name index is written to disk after the first build, after each compaction and on clean stop. On restart the daemon loads the saved index and re-diffs directories whose modified time is newer than the save. Compaction folds the overlay into a new base when the overlay reaches 50,000 entries or 12 hours have passed since the last one, and searches keep working during it.
45. `shift install` copies the running binary to `~/.local/bin/shift` by writing a temporary file next to it and renaming it into place, and stops a running daemon first. Without `--login` it prints the installed path and says the daemon starts on first use. With `--login` it also writes a macOS login agent to `~/Library/LaunchAgents/` that runs `shift serve` at login (label `local.shift.daemon`), restarts it if it exits, and appends its output to `daemon.log`, then loads it with `launchctl bootstrap` for the current user, retrying for up to 5 seconds. A reinstall without `--login` keeps an existing login agent.
46. `shift uninstall` unloads the login agent, deletes its file, prints what it removed, and keeps the index and configuration. It succeeds with exit status 0 when no agent is installed.

### J. Learning path (deliverables of the project itself)

47. The project contains a `learn/` folder with one lesson file per phase, named `00-` through `12-` in the order of the build phases in this blueprint. Each lesson states: the Rust concepts introduced (and only those not covered in earlier lessons), the goal of the phase, the step-by-step build with explanation of each part, one exercise the author does alone, and the tests that prove the phase is done.
48. Each phase ends with passing tests specific to that phase and a command the author can run to see the new capability working. No phase starts before the previous phase's tests pass.
49. `cargo test` passes, and `cargo clippy` reports no warnings, at the end of every phase.

## Constraints

- Language and toolchain: Rust, stable channel, set in `rust-toolchain.toml` as the stable channel (not an exact version). Edition 2021 or newer.
- No `unsafe` code, no memory-mapped files, no raw system-call bindings. Anything fsearch does with those is done with safe standard-library or listed-crate equivalents, and where no equivalent exists the feature is out of scope.
- Platform: macOS only (Apple silicon). Cross-platform support is not a goal. Full Disk Access is not requested by the program; unreadable folders are skipped and reported.
- Models: free and local only. Embeddings are `bge-m3` through Ollama. Speech to text is whisper.cpp through `whisper-cli`, model size `small` (multilingual, automatic language detection). No paid or hosted model is used or suggested anywhere.
- External programs are invoked as subprocesses and never re-implemented: `yt-dlp`, `ffmpeg`, `whisper-cli`, `launchctl`, and `id` (to get the user id). Ollama is called over local HTTP.
- No async runtime. The program is blocking code with standard-library threads and channels. This is a deliberate learning constraint.
- Dependencies are limited to this list: `clap`, `anyhow`, `serde`, `serde_json`, `toml`, `rusqlite` (with bundled SQLite and full-text search), `scraper`, `ureq`, `regex`, `memchr`, `ignore`, `notify`, `signal-hook`, and `tempfile` for tests only. Adding any other crate requires the author's explicit approval.
- Instagram access: only the author's own export files and their own logged-in browser session through `yt-dlp`. Downloads are throttled as in requirement 19. Automated downloading is against Instagram's terms and the author accepts that risk.
- Privacy: all data stays on the author's machine. Downloaded audio is stored under the root directory and may be deleted after transcription by a documented command; transcripts and captions are kept.
- Version control: git is read-only for the assistant. The assistant never stages, commits or pushes. The author performs all version-control actions.
- Learning mode: the author types the code. The assistant provides code, a part-by-part explanation and an exercise for each step, and does not edit the author's project files.
- Code style: simple names, no comments that restate code, comments only for non-obvious reasons, tests alongside the code they test.
- Out of scope, final: fsearch's `sym:` and `kind:` filters, its event-id replay across restarts (replaced by directory-modified-time catch-up), its per-file bloom filters and content deduplication, its performance tricks (custom allocator, keep-warm thread, I/O policy settings), a GUI or menu-bar app, Windows or Linux, remote access to the daemon, Instagram posting or any write action, OCR of video frames, and embedding the whole disk.

## Architecture Decisions

### 1. One crate, one binary

- **Decision:** A single package builds one `shift` binary. Modules are organized by responsibility: configuration and paths, database, embeddings, Instagram, name index, query language and scoring, content index, live index, daemon and protocol, install, and the command-line surface.
- **Rationale:** One package is the simplest Cargo setup for a beginner, and the two features share the embedding, output and path code.
- **Consequences:** No workspace to manage. The two features share code but not a query; there is no combined "search everything" command.

### 2. Command shape follows fsearch, built with clap

- **Decision:** `clap` defines the commands. Named commands are `insta`, `files`, `serve`, `stop`, `status`, `stdio`, `bench`, `install`, `uninstall`, `config`. Any other first word is a file-search query, using clap's mechanism for unrecognized subcommands, so a bare `shift <words>` searches.
- **Rationale:** fsearch treats an unrecognized first argument as the query and hand-parses a handful of commands. shift has nested commands and per-command flags where hand parsing would be long and error-prone, so the parser is generated while the behavior stays identical.
- **Consequences:** A typo in a command name becomes a search query rather than an error, exactly as in fsearch. `stop` is the one command added beyond fsearch's public set, because stopping the daemon must be testable.

### 3. Storage is split by shape of the data

- **Decision:** Instagram posts, chunks, vectors and file-semantic chunks live in one SQLite database. The name index and the trigram content index are custom flat files read fully into memory with safe code and never memory-mapped.
- **Rationale:** Posts and vectors are relational and need transactional resumability. The name and content indexes are read-mostly, ordered arrays where fsearch's layout is the lesson, and writing them by hand teaches byte-level formats, serialization and slices.
- **Consequences:** Start-up reads the whole name index into memory (acceptable at millions of entries, tens to hundreds of megabytes). Two storage mechanisms must be learned, in separate phases.

### 4. Name index layout copies fsearch's range idea

- **Decision:** Entries are stored one directory block at a time in depth-first order, children of each directory contiguous and sorted by name, so each directory's subtree is one index range. Distinct names are stored once and entries refer to them. Each distinct name carries a 64-bit mask of the characters it contains, and a name can only match a query if its mask covers every query character; only names passing the mask are scored.
- **Rationale:** Folder scoping becomes a range bound, interned names cut scoring work by a large factor, and the mask is a cheap, safe prefilter that does most of what fsearch's bitmaps do.
- **Consequences:** Inserting an entry in the middle is never done; changes go to the overlay (decision 6) until compaction rebuilds the layout.

### 5. Fuzzy matching and typo tolerance are written by hand

- **Decision:** The subsequence scorer (match, gap, boundary, camelCase and consecutive weights) and the one-edit typo matcher are implemented in the project, not taken from a fuzzy-matching crate.
- **Rationale:** They are small, self-contained, richly testable, and teach strings, iterators, slices and bounded edit distance.
- **Consequences:** Ranking quality depends on the author's scoring rules and tests.

### 6. Live index: base, removed set, overlay, idempotent directory diff

- **Decision:** The searchable state is an immutable base, a set of removed base entries, and a map of overlay entries keyed by path. Every file event is turned into "re-list this directory and diff it", and the diff is the only way state changes. Watching starts before the initial scan. Compaction rebuilds the base from base plus overlay minus removed.
- **Rationale:** Idempotent diffs make duplicate, reordered and replayed events harmless, which removes a whole class of concurrency bugs and is the single most reusable idea in fsearch.
- **Consequences:** A burst of changes to one directory costs one listing, not one operation per event. Restart catch-up uses directory modified times rather than event identifiers, which can re-list slightly more than strictly needed.

### 7. Content index: immutable trigram segments with verification

- **Decision:** Case-folded 3-byte sequences map to sorted lists of document identifiers, delta-encoded, written into immutable segment files together with a document table. Changed files go into a new segment, removed files are marked as dead, and segments are merged when more than eight exist. Candidates from the index are always verified by reading the file from disk.
- **Rationale:** It is the technique fsearch uses, immutability makes concurrent reading safe without locks on segment contents, and verification guarantees results are never stale.
- **Consequences:** Literals under 3 bytes are rejected rather than slowly scanned. The index is larger than minimal because there are no bloom filters or deduplication.

### 8. Directory walking: hand-written first, then replaced

- **Decision:** Phase 5 builds a single-threaded recursive walker with the standard library to learn paths and error handling. Phase 7 replaces it with the `ignore` crate's parallel walker with all ignore-file rules turned off and hidden entries included, and the hand-written walker is deleted.
- **Rationale:** The learning value is in the first version; correctness and speed on a real disk is better delegated.
- **Consequences:** Phase 5 results are slower and are not the shipped behavior. Walker tests written in phase 5 keep running against the replacement.

### 9. The daemon is mandatory for file search and started on first use

- **Decision:** All file-name, content and file-semantic queries go through the daemon. A client that cannot connect starts `shift serve` detached (re-executing the same binary with output going to the log), then retries connecting. One thread serves each client connection, one thread applies file events, and one thread runs content and embedding work, sharing state behind standard-library locks. Shutdown is driven by a termination signal or a stop request.
- **Rationale:** This is how fsearch behaves, it keeps one authoritative index, and it avoids two code paths to keep identical. Self-spawning avoids unsafe process-forking code.
- **Consequences:** The first query after a cold start waits for the daemon to start and may report "still indexing". `bench` is the only way to run the index without the daemon. Instagram commands are independent of the daemon.

### 10. Single owner by lock file

- **Decision:** The daemon takes an exclusive advisory lock on `daemon.lock` using the standard library's file locking and holds it until it exits.
- **Rationale:** The operating system releases the lock when the process dies for any reason, so there is no stale pid file to clean up and no race between two starters.
- **Consequences:** A second daemon exits quietly. The socket file may be left behind after a crash and is replaced on the next start.

### 11. Install and login agent follow fsearch's sequence

- **Decision:** Install first unloads any login agent (so the supervisor does not restart the old daemon), then stops the running daemon, copies the binary to `~/.local/bin/shift` through a temporary name and a rename (never overwriting in place), and, if `--login` was given or an agent already existed, writes the agent file and loads it with `launchctl bootstrap` for the current user, retrying every 100 ms for up to 5 seconds. Uninstall unloads and deletes the agent and keeps data.
- **Rationale:** The ordering and the retry are the lessons fsearch encodes: unload must precede stop, the unload call returns before the old job is fully gone, and replacing a running executable in place is unreliable.
- **Consequences:** Nothing is installed or registered unless the author types the command. The user id comes from running `id -u`, avoiding any system-call binding.

### 12. Vectors, ranking and embeddings (unchanged)

- **Decision:** Vectors are 1024 32-bit floats per chunk stored as BLOBs and searched by exhaustive cosine similarity with a heap of the best k. Vector and BM25 rankings are merged with reciprocal rank fusion, constant 60, for both Instagram and semantic file search.
- **Rationale:** At most about 130,000 chunks (30,000 Instagram plus the 100,000 file-chunk cap) is small enough for exhaustive comparison within the latency budget, and fusion needs no score calibration.
- **Consequences:** Search cost grows linearly with chunk count, protected by the cap in requirement 38.

### 13. Configuration and secrets (unchanged)

- **Decision:** One TOML file with settings for exclusions, embedding server, whisper model path, and limits. No secrets are stored; browser cookies are read by `yt-dlp` from the browser at download time.
- **Rationale:** Nothing sensitive is written to disk by shift itself.
- **Consequences:** If the browser's cookie store changes format, only `yt-dlp` needs updating.

### 14. Build order (the learning path)

- **Decision:** The project is built in these thirteen phases, in this order. Each phase introduces only the listed new Rust concepts.

| Phase | Deliverable | New Rust concepts |
|---|---|---|
| 0 | Cargo project, command shape with bare-query fallback, config file, root directory and `SHIFT_HOME` | packages and crates, modules, `main`, `Result`, `?`, structs, enums, `clap` derive |
| 1 | `insta import` into SQLite | `String` vs `&str`, `Option`, iterators, closures, `rusqlite`, unit tests, fixtures |
| 2 | Embedding client and `insta embed` | traits and `impl`, `serde`, HTTP with `ureq`, byte conversion, error context with `anyhow` |
| 3 | `insta search` with hybrid ranking | `Vec`, sorting, `HashMap`, binary heap, generics lite, full-text search SQL |
| 4 | `insta fetch` and `insta transcribe`, status tracking | `std::process::Command`, enums as state machines, `match`, idempotent resumable work |
| 5 | Hand-written walker, in-memory name index, fuzzy search in-process | recursion, `PathBuf`, lifetimes introduction, slices, ordering and `Ord` |
| 6 | Query language: filters and operators | parsing with enums, pattern matching depth, error types, property-style tests |
| 7 | Directory-range layout, interned names, character masks, saved index file, parallel walking, `bench` | byte-level serialization, `&[u8]`, threads, channels, `Arc`, the `ignore` crate |
| 8 | Content search: literal, regex, smart-case, then trigram segments with verification and merge | `regex`, `memchr`, sorted-list intersection, varint encoding, benchmarks |
| 9 | Semantic search for registered text directories | refactoring to share modules, chunking, generics for a shared embedder, capped resources |
| 10 | Daemon core: `serve`, lock file, socket, JSON-lines protocol, thin client, auto-start, `stdio`, `status`, `stop` | Unix sockets, `BufRead`, long-lived threads, `Mutex` and `RwLock`, signals, graceful shutdown |
| 11 | Live updates: watcher, idempotent directory diff, overlay, removed set, compaction, restart catch-up | `notify`, shared state design, debouncing, state machines under concurrency |
| 12 | `install` and `uninstall` with the login agent, performance tests, README | file generation, subprocess ordering and retries, profiling basics, documentation |

- **Rationale:** The Instagram path is built first because it is smaller and already has real data, so the author sees a working tool early. The file search follows in-process, so every algorithm is testable before concurrency exists. The daemon comes after the algorithms, then live updates, because each needs the layer before it.
- **Consequences:** The author has a useful Instagram search after phase 3, in-process file search after phase 8 (through `bench`), and the fsearch-style daemon experience only from phase 10. Until phase 10, file search is run through `bench`, which is intentional.

### 15. Teaching format

- **Decision:** Each lesson in `learn/` follows the same structure (concepts, goal, steps with explanation, exercise, tests) and is delivered to the author as markdown before they type any code. The assistant reviews the author's code on request but does not apply edits.
- **Rationale:** Typing the code is the learning mechanism; a consistent lesson format keeps the sequence navigable.
- **Consequences:** Progress is paced by the author, and the assistant's role is explanation and review, not implementation.

## Verification

Each entry maps to the requirement with the same number.

1. Run `cargo build` in `~/Developer/gyaan/shift/`: it succeeds with no warnings, and `target/debug/shift` exists. Search the source for the keyword `unsafe`: no occurrences.
2. Run `shift --help`: the ten commands are listed and no others. Run `shift` with a first word that is not a command: it is treated as a query (it answers, or reports "indexing" through the daemon path, and does not print a usage error). Run `--help` on each subcommand: every flag in this blueprint appears.
3. Run any command with `SHIFT_HOME` set to a temp directory: all files created are inside it, and the real `~/Library/Application Support/Shift/` is untouched. With it unset, the root resolves to the default.
4. Run `shift config show` with no config file: defaults print and no file is created. Create a config file and change one value: `config show` reflects it. `config path` prints that file's location.
5. Run each result-printing command with `--json`: stdout is parseable line by line as JSON with no extra text. Force a runtime error and a usage error: exit statuses are 1 and 2.
6. Import a small fixture export: each post has all listed fields, and kind and collection are correct against hand-checked values.
7. Import the real export: counts are 2,493 posts, 2,141 reels, 352 posts, and wall time is under 10 seconds.
8. Import the same export twice: post count unchanged, a hand-edited caption in the database is overwritten by the export value, and a post's status set to `embedded` before the second import is still `embedded`.
9. Run import on an empty directory and on a file with no entries: exit status 1, message names the problem, database unchanged.
10. Inspect configuration defaults and capture network traffic or logs during `embed`: the only host contacted is the configured local Ollama address.
11. Run `embed`, interrupt it midway, re-run: the second run embeds only the remainder. At completion, 2,470 caption chunks have vectors and the count of chunks without vectors is zero.
12. Stop Ollama and run `embed`: status 1 with the "not reachable" message. Run with a model name that is not installed: status 1 with the "model missing" message and the pull command. The database has no new rows from the failed run.
13. Write a known vector to the database and read it back: all 1024 values are bit-identical.
14. Run a search for a phrase taken from a known caption with the daemon stopped: that post is in the top 3. Run a paraphrase of the same caption: the post is in the top 10. Each post appears at most once in the list.
15. Inspect the output of one reel result and one regular-post result: all listed fields are present, and transcript results include a time range.
16. Run a search with each filter and with combinations: every result satisfies every filter, and when at least 10 posts satisfy them, exactly 10 are returned.
17. Time 100 searches with embedded query vectors cached against 30,000 chunks: median under 150 ms.
18. Search a nonsense string: exit status 0, one line saying no matches, no stack trace.
19. Run `fetch --limit 3` on a clean database: three audio files exist, at least 5 seconds apart by timestamp, none repeated on a second run. Simulate a rate-limit response with a stub program in place of `yt-dlp`: the run stops at once and reports it.
20. Include a deleted post URL in a fixture: it ends as `failed` with an error text, is skipped by the next `fetch`, is included with `--retry-failed`, and its caption is still found by search.
21. Run `transcribe` on a known 60-second audio fixture: chunks are between 30 and 60 seconds, boundaries fall on segment ends, and a spoken phrase from the fixture is found by `insta search` after `embed`.
22. Run `insta stats` after a mixed run: counts per status sum to the total post count, and failed posts show grouped reasons.
23. Kill each of `fetch`, `transcribe` and `embed` with a hard kill at a random point, repeat five times, then run it to completion: the final state equals an uninterrupted run, and SQLite integrity check passes.
24. With no daemon running, run `shift <name>` against a fixture root: the daemon starts, and once ready the expected paths are listed with size and modified time. Stop the daemon, delete the index, start a build over a large tree and query immediately: status 1 with a message containing an entry count that increases between two attempts.
25. Run a fixed list of queries against a fixture tree with a hand-written expected order: outputs match exactly, including ties. Check that a 5-letter term with one typo matches and a 4-letter term with one typo does not. Check that a camelCase-boundary match outranks a mid-word match of the same characters.
26. Run one query per filter and per operator against a fixture tree: results match a hand-computed expected set. Run a query with an unknown filter: status 2 and the filter name in the message. Check that an `in:` query on a tree with 100 sibling directories visits only the target directory's entries (observable through a debug counter in `bench` output).
27. Index a tree containing every excluded directory name and an unreadable directory: excluded names are absent and the unreadable one is counted in `status`. Revoke Full Disk Access (or start with it absent): the log states that gated folders are skipped and `status` shows the state.
28. Generate a 200,000-file tree: full index completes in under 60 seconds, and 1,000 name queries against a warm daemon have a 95th percentile under 20 ms.
29. Run `grep:` with a lowercase pattern and with a pattern containing a capital against a fixture containing both cases: the first matches both, the second matches only the exact case. Output lines include path, line number and text.
30. Run a valid and an invalid regular expression: the valid one returns expected lines, the invalid one exits with status 2 and prints the parse error.
31. Add a binary file, a file of 2 MiB, and a text file to a fixture tree: only the text file is content-searchable, and `status` reports the other two as skipped.
32. Run a 3-byte and a longer literal: both return results and every result line actually exists in the file on disk. Edit a file after indexing without touching the index, then search for the new text: it is found because verification reads the file, and old removed text is not returned. Run a 2-byte literal: status 2 with the minimum-length message.
33. Run a regex with an extractable literal and one without: the first reports indexed use, the second prints the unindexed scan note, and both return the same lines as `rg` over the same tree.
34. On a generated tree of 20,000 text files, run 200 indexed literal searches with under 50 matching files each: 95th percentile under 100 ms, and every result set equals the `rg` result set.
35. Register, list and remove a directory: only registered directories have chunks in the database, and removing one deletes its chunks.
36. Embed a fixture directory with files of listed and unlisted extensions and one oversized file: only listed, small files have chunks, chunks are about 1,200 characters, overlapping by 150, and begin at line starts.
37. Search semantically for a paraphrase of a sentence in a fixture markdown file: that file is in the top 10, one result per file, with path and starting line, and the request is visible in the daemon log.
38. Touch a file without changing it, change another, delete a third, then re-run: only the changed file is re-embedded, the deleted file has no chunks, and the unchanged file's chunk identifiers are unchanged. Set the cap to a tiny value: indexing stops with the message and status 1.
39. Run `serve` in one terminal and `status` in another: all listed fields are present. Run `stop`: the daemon exits within 5 seconds and the socket file is gone.
40. Run `serve` twice: the second prints already running with status 0 and only one process exists. Kill the daemon with a hard kill, leaving the socket: the next client command starts a new daemon that works, with no manual cleanup.
41. Run `shift <query>` with the daemon stopped and observe it start the daemon. Run `shift bench <query>` and compare its 10 results with the daemon's for the same query and index state: identical paths in identical order, and the timing line has first, median and minimum.
42. Send a handwritten search request to the socket with a generic socket tool: result lines and a final end line come back as documented. Send a malformed request: an error response line, and the connection stays usable. Pipe two requests through `shift stdio`: two answers come back in order.
43. With the daemon running, create, rename, modify and delete files in an indexed root: each change is reflected in name or content search within 2 seconds, measured by polling. Create 5,000 files in one burst: all appear and the daemon stays responsive. Replay the same directory re-list twice: counts in `status` do not change. Create files during a large initial scan: they are present once the scan completes.
44. Delete 10 files and create 10 others: `status` shows removed and overlay counts of 10 each. Stop and restart: the counts survive a clean stop, and files changed while the daemon was down are found after the catch-up. Set the compaction threshold low in a test: overlay and removed counts return to zero after compaction, and a query repeated during compaction returns the same answer.
45. Run `install` without `--login` using a test home directory: the binary exists at the expected path, no agent file exists, and the output says the daemon starts on first use. Run it with `--login`: the agent file exists with the label, restart-on-exit and log settings, and `launchctl` lists the job. Run `install` again without the flag: the agent file is still present.
46. Run `uninstall` after a login install: the job is unloaded, the file is gone, the message names what was removed, and the index and configuration still exist. Run it again: exit status 0.
47. List `learn/`: files `00-` through `12-` exist, each containing all five parts. Check that no lesson introduces a concept listed in an earlier lesson's table row as new.
48. For each phase, run the phase's named demo command and its tests on the finished phase: tests pass and the capability works. The next phase's folder or module does not exist before this check passes.
49. Run `cargo test` and `cargo clippy` at the end of each phase: zero failures and zero warnings.
