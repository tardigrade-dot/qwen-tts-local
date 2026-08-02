# Qwen3-TTS C → Rust Front-End

## Agent Task Brief & Architecture Guide

> **Audience:** Coding agent  
> **Upstream C repo:** https://github.com/tardigrade-dot/qwen3-tts-c  
> **Primary headers (source of truth):** `qwen_tts.h`, `qwen_tts_emotion.h`, `qwen_tts_compose.h`, `qwen_tts_audio.h`, `qwen_tts_voice_clone.h`, `qwen_tts_thread.h`, `qwen_tts_server.h`  
> **Reference consumers in C:** `main.c` (CLI), `qwen_tts_server.c` (HTTP)

**Read this document in order.** Section A defines *what users can do* (from real C CLI/server behavior). Section B lists *every callable C API* you may use. Section C states engineering constraints (including **no C source edits**). Sections D–F cover architecture, phases, and acceptance.

---

# A. Product surface — usage modes to reimplement in Rust

Rust must provide **two front-ends** that call the same C inference library:

| Mode | Role | C reference |
|------|------|-------------|
| **CLI** | One-shot / streaming synthesis to file or stdout; voice mgmt helpers | `main.c` |
| **HTTP serve** | Long-running process; model stays loaded across requests | `qwen_tts_server.c` + `qwen_tts_serve*` |

Do **not** call `qwen_tts_serve` / `qwen_tts_serve_ex` / `qwen_tts_serve_batched` from Rust. Reimplement HTTP in Rust; only reuse inference + shared helpers (emotion/compose/audio).

Audio output contract (both modes):

- Sample rate **24000 Hz** (`QWEN_TTS_SAMPLE_RATE`)
- Internal PCM from C: **float32 mono**, ≈ `[-1, 1]`
- On-disk / wire streaming: **s16le mono** (WAV or raw PCM)

---

## A.1 CLI usage modes (from `main.c`)

### A.1.1 Core synthesis (required MVP)

```text
qwen_tts -d <model_dir> --text "..." [-o out.wav]
  [-s speaker] [-l language]
  [-T temperature] [--top-k N] [--top-p F] [--rep-penalty F]
  [--seed N] [--max-tokens N] [--max-duration SECS]
  [-j threads] [--int8|--int4] [--silent]
```

**C call sequence (simplified):**

1. `qwen_check_runtime_isa()` (from kernels; optional but CLI does it)
2. `qwen_set_threads(n)` or `qwen_init_threads()`
3. `ctx = qwen_tts_load_ex(model_dir, silent, use_int8, use_int4)`
4. Set fields / helpers: speaker, language, sampling, `instruct`, etc.
5. Either:
   - **Non-stream:** `qwen_tts_generate(ctx, text, &samples, &n)` → free samples after use → `qwen_tts_write_wav(...)` **or** build WAV in Rust
   - **Stream:** set `ctx->stream = 1`, `ctx->stream_chunk_frames`, `qwen_tts_set_audio_callback(...)`, then `qwen_tts_generate` (callback receives chunks; CLI writes s16le to WAV/stdout)
6. `qwen_tts_unload(ctx)`

Defaults used by CLI/server when not overridden:

| Param | Default |
|-------|---------|
| speaker | `ryan` (id `3061`) |
| language | `English` (id `2050`) |
| temperature | `0.5` |
| top_k | `50` |
| top_p | `1.0` |
| rep_penalty | `1.05` |
| seed | time-based if unset |
| stream_chunk_frames | `10` (≈ 0.8 s audio at 12.5 Hz) |

Preset speakers (CustomVoice): `ryan`, `aiden`, `vivian`, `serena`, `uncle_fu`, `dylan`, `eric`, `ono_anna`, `sohee`.  
Languages: English, Chinese, Japanese, Korean, German, French, Russian, Portuguese, Spanish, Italian.  
Resolve with `qwen_tts_speaker_id` / `qwen_tts_language_id` (returns negative if unknown).

### A.1.2 Streaming CLI

| Flag | Behavior |
|------|----------|
| `--stream` | Decode during generation; write progressive audio |
| `--stdout` | Implies `--stream`; write **raw s16le** to stdout (no WAV header) |
| `--stream-chunk N` | Frames per callback chunk |

Callback contract (must implement cancel this way):

```c
typedef int (*qwen_tts_audio_cb)(const float *samples, int n_samples, void *userdata);
/* return 0 = continue, non-zero = abort generation */
```

### A.1.3 Style / emotion / markup (shared with server)

| Capability | CLI | C API / mechanism |
|------------|-----|-------------------|
| Free-form style | `--instruct "..."` (1.7B) | `ctx->instruct = strdup(...)`; free previous |
| Named emotion | `--emotion <name>` | `qwen_tts_apply_emotion(...)` |
| Volume / rate DSP | `--volume`, `--rate` | `qwen_audio_apply_gain`, `qwen_audio_time_stretch` (rate often post full buffer) |
| Roughness | `--roughness` | `ctx->cp_roughness` |
| Inline markup | auto in `--text` or `--compose` | `qwen_compose_has_markup` → parse → `qwen_compose_render_buffer` / `_stream` |
| Disable auto markup | `--no-compose` | skip compose path |

Markup tags (composer): `[joy]`/`[sad]`/…, `[neutral]`, `[pause:400ms]`/`[break:1s]`, paralinguistics `[laugh]`/`[sigh]`/…  
Paralinguistic substitution: `qwen_compose_para_substitute` (CLI/server both call before generate when tags present).

### A.1.4 Voice clone / profiles (CLI-heavy; server usually loads at startup)

| Flag | Meaning |
|------|---------|
| `--ref-audio WAV` | Base model only; 24 kHz mono WAV required |
| `--ref-text` | ICL reference transcript |
| `--xvector-only` | Embedding only (no ICL codes) |
| `--save-voice PATH` | `.qvoice` graft (~16–25 MB) or `.bin` x-vector (~8 KB) |
| `--load-voice PATH` | Load profile |
| `--icl-only` | Keep CustomVoice weights; use ICL prefix from `.qvoice` (instruct/emotion still work) |
| `--graft` | Implies `--icl-only`; x-vector path, emotive |
| `--list-voices DIR` | List `.qvoice` (no model required in C CLI) |
| `--delete-voice PATH` | Delete file |
| `--voice-design` | 1.7B VoiceDesign model; voice from `--instruct` |

Much of `.qvoice` load/save logic lives **in `main.c`**, not as a single public `qwen_tts_load_voice()` API. For Rust MVP: either (1) defer full `.qvoice` parity, or (2) carefully reimplement using `qwen_extract_speaker_embedding` / speech encoder APIs + documented file format. Do not invent C functions that do not exist.

### A.1.5 Long-form batch (CLI)

| Flag | Behavior |
|------|----------|
| `--batch` | Split long text into sentence-packed chunks |
| `--batch-words N` | Target words per chunk (default 16) |
| `--batch-dry` | Print chunks only |

C path: split in CLI → `qwen_tts_generate_batch` (falls back to sequential if returns `-2`).

### A.1.6 Quantization & threads

| Flag | C |
|------|---|
| `--int8` / `--int4` | `qwen_tts_load_ex(..., use_int8, use_int4)` |
| `--quant-mixed` | int4 Talker + env `QWEN_CP_PREC=int8` (CLI-specific) |
| `-j N` | `qwen_set_threads(N)` |

### A.1.7 HTTP from the same binary (C only — do not port this entry)

C CLI also supports `--serve PORT`, `--workers N`, `--batch-size N` by calling `qwen_tts_serve*`. **Rust HTTP replaces this**; do not link/call those functions for the new server.

### A.1.8 CLI flags that are out of scope for early Rust phases

Dev/bench/GPU: `--self-test`, `--matmat-bench`, `--gpu-selftest*`, `--backend`, `--caps`, `--batch-test`, `--batch-bench`, `--batch-multi-test`, `--seed-audition`, `--onset-fade`, `--tail-trim`, advanced `--ml-steer` / `--expr` wiring.

Implement core synthesis + stream + emotion/compose **before** these.

---

## A.2 HTTP serve usage modes (from `qwen_tts_server.c`)

### A.2.1 Endpoints (parity target)

| Method | Path | Response |
|--------|------|----------|
| `GET` | `/v1/health` | `{"status":"ok"}` (+ Rust may add `loaded`, etc.) |
| `GET` | `/v1/speakers` | JSON list of 9 preset speakers |
| `POST` | `/v1/tts` | Full **WAV** body |
| `POST` | `/v1/tts/stream` | **Chunked s16le PCM**; headers: `Content-Type: audio/pcm`, `X-Sample-Rate: 24000`, `X-Sample-Format: s16le`, `X-Channels: 1` |
| `POST` | `/v1/audio/speech` | OpenAI-compatible; `input`→text, `voice`→speaker; WAV |

Max text length in C server: **8192** chars (`MAX_TTS_TEXT`).

### A.2.2 JSON body fields (server)

| Field | Maps to |
|-------|---------|
| `text` or `input` | synthesis text |
| `speaker` or `voice` | `qwen_tts_speaker_id` |
| `language` | `qwen_tts_language_id` |
| `instruct` | `ctx->instruct` |
| `voice_design` | `ctx->voice_design` (`"true"`/`"1"`) |
| `temperature`, `top_k`, `top_p`, `rep_penalty` | ctx sampling (clamped) |
| `seed` | `ctx->seed` if ≥ 0 |
| `emotion` | `qwen_tts_apply_emotion` |
| `volume`, `rate` | post DSP (`qwen_audio_apply_gain` / `time_stretch`; **rate not applied on stream path** in C) |

### A.2.3 Server per-request state reset (mandatory)

From `reset_request_state` in `qwen_tts_server.c`:

- If **not** `ctx->voice_clone`: speaker=`3061`, language=`2050`
- If voice_clone: keep voice metadata language/speaker
- `temperature=0.5`, `top_k=50`, `top_p=1.0`, `rep_penalty=1.05`
- `voice_design=0`; free `instruct`
- Clear `cp_roughness`; free `ml_steer`
- New time-based seed unless request supplies one

**Without this, parameters leak across HTTP requests.**

### A.2.4 Server generation paths

**Full WAV (`/v1/tts`):**

1. `parse_tts_request` (reset + JSON + optional para substitute + emotion)
2. `ctx->stream = 0`; `audio_cb = NULL`
3. If `qwen_compose_has_markup(text)` → `qwen_compose_parse` → `qwen_compose_render_buffer`
4. Else → `qwen_tts_generate`
5. Apply volume/rate DSP on full buffer
6. Build WAV in memory → HTTP response
7. `free(audio)`

**Stream (`/v1/tts/stream`):**

1. Same parse
2. If markup → `qwen_compose_render_stream` with chunk callback → HTTP chunked s16le
3. Else → `ctx->stream = 1`, `qwen_tts_set_audio_callback` → HTTP chunked s16le
4. Volume applied **per chunk** in callback; rate skipped on stream in C

### A.2.5 Server concurrency notes (C)

- Shared mutable `ctx` protected by mutex around synth
- `--workers N`: `qwen_tts_clone_for_worker` per worker; if `!qwen_parallel_is_reentrant()`, serialize synth
- `--batch-size N`: continuous batching via `qwen_tts_serve_continuous` — **optional late phase**; not required for MVP

### A.2.6 Rust enhancements (allowed; not in C server)

These are product goals on top of C parity:

1. **Cancel in-flight synthesis** on client disconnect (callback returns non-zero)
2. **Streaming events** that include **text spans + audio** (C only streams audio; text alignment is Rust-side split or compose spans)
3. **Idle unload** of the model to free RAM (`qwen_tts_unload` when idle and not busy)

---

# B. Complete C API inventory (callable from Rust)

Only use symbols declared in public headers. Signatures below are taken from those headers.

## B.1 Core lifecycle & generation — `qwen_tts.h`

```c
qwen_tts_ctx_t *qwen_tts_load(const char *model_dir);
qwen_tts_ctx_t *qwen_tts_load_ex(const char *model_dir, int silent, int use_int8, int use_int4);
void            qwen_tts_unload(qwen_tts_ctx_t *ctx);

void            qwen_track_override(qwen_tts_ctx_t *ctx, void *ptr);

qwen_tts_ctx_t *qwen_tts_clone_for_worker(const qwen_tts_ctx_t *base);
void            qwen_tts_free_clone(qwen_tts_ctx_t *ctx);
/* NEVER call unload on a clone — only free_clone */

void qwen_tts_set_speaker(qwen_tts_ctx_t *ctx, int speaker_id);
void qwen_tts_set_language(qwen_tts_ctx_t *ctx, const char *language);
int  qwen_tts_language_id(const char *name);  /* negative if unknown */
int  qwen_tts_speaker_id(const char *name);   /* negative if unknown */

typedef int (*qwen_tts_audio_cb)(const float *samples, int n_samples, void *userdata);
void qwen_tts_set_audio_callback(qwen_tts_ctx_t *ctx, qwen_tts_audio_cb cb, void *userdata);

int qwen_tts_generate(qwen_tts_ctx_t *ctx, const char *text,
                      float **out_samples, int *out_n_samples);
/* out_samples is malloc'd by C; caller must free() */

int qwen_tts_generate_batch(qwen_tts_ctx_t *ctx, char **chunks, int nc,
                            float chunk_pause, float **out_samples, int *out_n_samples);
/* returns -2 if batched path unavailable */

int qwen_tts_generate_batch_multi(qwen_tts_ctx_t *ctx,
                                  const qwen_batch_req_t *reqs, int nc,
                                  float **out_samples, int *out_n_samples);

int qwen_tts_serve_continuous(qwen_tts_ctx_t *ctx, int max_batch, qwen_batch_sink_t *sink);
/* optional; prefer not for MVP */

int qwen_tts_write_wav(const char *path, const float *samples, int n_samples, int sample_rate);

int qwen_speech_encoder_load(qwen_tts_ctx_t *ctx);
int qwen_speech_encoder_encode(qwen_tts_ctx_t *ctx, const float *audio, int n_samples,
                               int **codes_out, int *n_frames_out);
```

**Constants:**

```c
#define QWEN_TTS_SAMPLE_RATE  24000
#define QWEN_TTS_FRAME_RATE   12.5
#define QWEN_TTS_HOP_SAMPLES  1920
```

**Important `qwen_tts_ctx_t` fields set by CLI/server (bindgen will expose if full struct is included):**

| Field | Purpose |
|-------|---------|
| `temperature`, `top_k`, `top_p`, `rep_penalty` | sampling |
| `max_tokens` | generation limit |
| `seed` | RNG |
| `speaker_id`, `language_id` | identity |
| `instruct` | malloc'd string; free before replace |
| `voice_design` | 0/1 |
| `voice_clone`, `xvector_only`, `ref_audio_path`, `ref_text`, `speaker_embedding`, … | clone |
| `stream`, `stream_chunk_frames`, `audio_cb`, `audio_cb_userdata` | streaming |
| `silent`, `debug`, `use_int8`, `use_int4` | load/runtime |
| `ml_steer`, `ml_steer_layers`, … | emotion steer buffer |
| `cp_roughness` | texture knob |
| `greedy_warmup` | stability |
| `is_base_model`, `config.hidden_size` | 0.6B vs 1.7B (`hidden_size` 1024 vs 2048) |

## B.2 Emotion — `qwen_tts_emotion.h`

```c
int qwen_tts_apply_emotion(qwen_tts_ctx_t *ctx,
        const char *emotion_spec, const char *language,
        float ro, int ro_set,
        float vo, int vo_set, float ra, int ra_set,
        float *out_volume, float *out_rate, int silent);

const char *qwen_emotion_name_to_tok(const char *name);
const char *const *qwen_emotion_steer_names(int *count);
int qwen_emotion_steer_install(qwen_tts_ctx_t *ctx, const char *tok,
                               float weight, int l0, int l1, int silent);
```

## B.3 Compose (inline markup) — `qwen_tts_compose.h`

```c
int  qwen_compose_has_markup(const char *text);
int  qwen_compose_has_para_event(const char *text);
int  qwen_compose_is_para_event_tag(const char *tag);

char *qwen_compose_para_substitute(const char *text, int voice_class,
                                   int *did, int *seed, float *temp);
/* returns malloc'd string; caller frees */

int  qwen_compose_parse(const char *input, qwen_cspan_t **out, int *out_n);
void qwen_compose_free_spans(qwen_cspan_t *spans, int n);

int  qwen_compose_render_buffer(qwen_tts_ctx_t *ctx, qwen_cspan_t *spans, int nspans,
                                const char *language, float default_pause,
                                float **out_audio, int *out_n, int silent);

typedef void (*qwen_compose_chunk_cb)(const float *pcm, int n, void *user);
int  qwen_compose_render_stream(qwen_tts_ctx_t *ctx, qwen_cspan_t *spans, int nspans,
                                const char *language, float default_pause,
                                qwen_compose_chunk_cb cb, void *user, int silent);
```

## B.4 Audio DSP / WAV — `qwen_tts_audio.h`

```c
int  qwen_tts_write_wav(const char *path, const float *samples, int n_samples, int sample_rate);
void qwen_audio_apply_gain(float *samples, int n_samples, float gain);
int  qwen_audio_time_stretch(const float *in, int n_in, float rate, int sample_rate,
                             float **out, int *out_n);
int  qwen_audio_first_onset(const float *s, int n, int sample_rate);
void qwen_audio_onset_fade(float *s, int n, int sample_rate, int fade_ms);
float qwen_audio_tail_glitch_score(const float *s, int n, int sample_rate, int *out_trim_at);
int  qwen_audio_tail_trim(float *s, int *n, int sample_rate, float min_score);
```

## B.5 Voice clone helpers — `qwen_tts_voice_clone.h`

```c
int  qwen_read_wav(const char *path, float **out_samples, int *out_n_samples, int *out_sample_rate);
void qwen_trim_trailing_silence(float *audio, int *n_samples, int sample_rate, int silent);
int  qwen_mel_spectrogram(const float *audio, int n_samples, int sample_rate,
                          float **out_mel, int *out_n_frames);
int  qwen_speaker_encoder_load(qwen_speaker_encoder_t *enc, void *safetensors);
int  qwen_speaker_encoder_forward(qwen_speaker_encoder_t *enc,
                                  const float *mel, int n_frames, float *out_embedding);
int  qwen_extract_speaker_embedding(qwen_tts_ctx_t *ctx, const char *ref_audio_path,
                                    float *out_embedding);
```

## B.6 Threading — `qwen_tts_thread.h` (+ kernels)

```c
void qwen_parallel(size_t nt, qwen_task_fn fn, void *ctx);
void qwen_threadpool_start(int n_threads);
void qwen_threadpool_stop(void);
int  qwen_parallel_is_reentrant(void);
```

CLI also calls `qwen_init_threads` / `qwen_set_threads` / `qwen_check_runtime_isa` (declared via `qwen_tts_kernels.h` — verify exact names in that header when binding).

## B.7 C HTTP server API — `qwen_tts_server.h` (**do not use for Rust server**)

```c
int qwen_tts_serve(qwen_tts_ctx_t *ctx, int port);
int qwen_tts_serve_ex(qwen_tts_ctx_t *ctx, int port, int n_workers);
int qwen_tts_serve_batched(qwen_tts_ctx_t *ctx, int port, int max_batch);
```

These implement the **entire** HTTP stack in C. Rust replaces them.

## B.8 What is **not** a public API

- Functions static to `main.c` (`.qvoice` file format read/write, batch text splitter, etc.)
- Internal Talker/decoder step functions used only by GPU self-tests
- Any symbol not declared in a public header

If a feature exists only as static logic in `main.c`, either reimplement in Rust or call a **public** helper that already encapsulates it (e.g. compose/emotion).

---

# C. Engineering constraints

1. **Do not modify C inference source** for features. Allowed: build glue only (compile selected `.c` into a static lib from `build.rs`, omit `main.c` and preferably omit `qwen_tts_server.c`).
2. **Do not reimplement** the neural pipeline in Rust.
3. **Do not call** `qwen_tts_serve*`; implement HTTP in Rust.
4. Inference is **blocking**. Use `spawn_blocking` / dedicated pool.
5. Ownership:
   - Base: `load*` → `unload`
   - Clone: `clone_for_worker` → `free_clone` only
   - `generate` / compose output buffers: `free` after copy
6. Prefer CPU+BLAS first; CUDA/Metal later.
7. Sources to compile into the lib: all inference TUs from upstream Makefile **except** `main.c` (and optionally `qwen_tts_server.c`). Include `vendor/lz4.c`. Compile `qwen_tts_speech_encoder.c` **without** `-ffast-math` (upstream special-case).
8. Link: Linux `-lopenblas -lm -lpthread`; macOS Accelerate.

---

# D. Target architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Rust binary (clap) — same UX shape as upstream `qwen_tts`    │
│                                                               │
│  DEFAULT (no subcommand): one-shot / stream synthesis         │
│    qwen-tts -d <model> --text "..." [-o out.wav] [--stream]   │
│                                                               │
│  Serve mode: flag or optional subcommand (both OK)            │
│    qwen-tts -d <model> --serve 8080                           │
│    qwen-tts serve -d <model> --port 8080   # equivalent       │
│                                                               │
│  Do NOT invent a required `synth` subcommand. Upstream has    │
│  no `synth`; synthesis is the default invocation.             │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│  Shared Rust core                                             │
│  ModelManager: load/unload/idle, busy, concurrent policy      │
│  request → reset state → emotion/compose/generate             │
│  Cancel flag + audio callback bridge                          │
└──────────────────────────────┬──────────────────────────────┘
                               │ spawn_blocking
┌──────────────────────────────▼──────────────────────────────┐
│  qwen-tts-sys (bindgen + cc)                                  │
│  Section B APIs only                                          │
└──────────────────────────────┬──────────────────────────────┘
                               │
┌──────────────────────────────▼──────────────────────────────┐
│  Unmodified C inference library                               │
└─────────────────────────────────────────────────────────────┘
```

**CLI shape (must match upstream mental model):**

| Invocation | Meaning |
|------------|---------|
| `qwen-tts -d DIR --text "..." -o out.wav` | Default: synthesize once (like C `qwen_tts`) |
| `qwen-tts -d DIR --text "..." --stream` / `--stdout` | Streaming synthesis |
| `qwen-tts -d DIR --serve 8080` | HTTP server (C uses `--serve`; Rust may also accept `serve` subcommand **as an alias only**) |

There is **no** upstream `synth` subcommand. Do not require `qwen-tts synth ...` as the primary interface.

Suggested layout:

```text
qwen3-tts-rust/
├── crates/qwen-tts-sys/     # FFI
└── crates/qwen-tts/         # single binary: default synth + --serve
```

---

# E. Phased implementation

### Phase 0 — FFI smoke (CLI core only)

- `build.rs` compiles C sources (no `main.c` / no C server required)
- bindgen allowlist for Section B.1 minimum
- CLI (default, **no** `synth` subcommand): `-d DIR -t TEXT -o out.wav` → non-silent 24 kHz WAV
  - Example: `qwen-tts -d "$QWEN_TTS_MODEL_DIR" --text "Hello" -o out.wav`
- `load_ex` → set speaker/language/sampling fields → `generate` → free → `unload`

**Accept:** plays back; process exits cleanly.

### Phase 1 — CLI streaming + HTTP minimal

- CLI `--stream` / `--stdout` via audio callback (still default command, not a subcommand)
- HTTP via `--serve PORT` (or optional `serve` subcommand alias only)
- HTTP routes: `GET /health`, `GET /speakers`, `POST /v1/tts` (WAV)
- Per-request reset (§A.2.3)
- Single-flight mutex OK

### Phase 2 — HTTP stream + cancel + idle unload

- `POST /v1/tts/stream` chunked s16le
- Disconnect → callback returns non-zero
- `ModelManager` idle unload (`--idle-secs`)
- CLI remains working

### Phase 3 — Emotion / compose parity

- Wire `qwen_tts_apply_emotion`, compose parse/render/stream
- HTTP body fields `emotion`, `volume`, `rate`, `instruct`
- Markup auto-detect like C server

### Phase 4 — Text+audio events + OpenAI route

- SSE/WS: text span events + audio (use compose spans or sentence split)
- `POST /v1/audio/speech`

### Phase 5 — Optional

- Worker clones + `qwen_parallel_is_reentrant` policy
- Long-form `generate_batch`
- Voice load/save (hard; much logic is in `main.c`)

---

# F. Agent operating rules

1. Prefer **usage parity with Section A** over inventing new APIs.
2. Only call **Section B** symbols; if something is missing, report — do not patch C.
3. Implement phases in order; write `PHASE_N_NOTES.md` after each.
4. Gate integration tests on `QWEN_TTS_MODEL_DIR`.
5. When reading struct fields, trust vendored `qwen_tts.h`, not guesses.
6. Cancel = non-zero from `qwen_tts_audio_cb`; there is **no** separate `qwen_tts_cancel()`.

---

# G. Definition of done

- [ ] Rust **CLI** synthesizes WAV (and optional stream/stdout) via C FFI
- [ ] Rust **HTTP serve** implements health, speakers, `/v1/tts`, `/v1/tts/stream`
- [ ] Cancel on disconnect works for streaming
- [ ] Idle unload works when configured
- [ ] No C source changes for features; no use of `qwen_tts_serve*`
- [ ] README: build deps, model download, CLI examples, HTTP examples

---

*Source of truth: upstream headers and `main.c` / `qwen_tts_server.c` as of the analysis date. If upstream drifts, re-read those files before changing bindings.*

----

最终验证方法:

server-customvoice 启动方式:

qwen_tts -d /Users/larry/Documents/sw_models/Qwen3-TTS-12Hz-1.7B-CustomVoice --int8 --serve "8092"

client测试方式, 验证vivian_test.wav文件存在: 
curl http://localhost:8092/v1/audio/speech \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen3-TTS-12Hz-0.6B-Base",
    "input": "总之，魏玛共和国政府的不稳定往往被夸大了，政府的频繁更迭掩盖了某些部门长期保持的连续性。",
    "response_format": "wav",
    "voice": "vivian",
    "emotion":"calm"
  }' \
  --output vivian_test.wav

server-base 启动方式:
./target/release/qwen-tts -d /Users/larry/Documents/sw_models/Qwen3-TTS-12Hz-0.6B-Base --ref-audio /Users/larry/Documents/resources/qinsheng-4s-24k.wav --int8 --serve "8092"

client测试方式, 验证qinsheng_test.wav文件存在: 
curl http://localhost:8092/v1/audio/speech \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen3-TTS-12Hz-0.6B-Base",
    "input": "总之，魏玛共和国政府的不稳定往往被夸大了，政府的频繁更迭掩盖了某些部门长期保持的连续性。",
    "response_format": "wav",
  }' \
  --output qinsheng_test.wav

cli测试base, 验证base_output.wav文件存在
./target/release/qwen-tts -d /Users/larry/Documents/sw_models/Qwen3-TTS-12Hz-0.6B-Base --ref-audio /Users/larry/Documents/resources/qinsheng-4s-24k.wav --int8 --text "总之，魏玛共和国政府的不稳定往往被夸大了，政府的频繁更迭掩盖了某些部门长期保持的连续性" -o base_output.wav

cli测试CustomVoice, 验证uncle_fu_test.wav文件存在
./target/release/qwen-tts -d /Users/larry/Documents/sw_models/Qwen3-TTS-12Hz-1.7B-CustomVoice -s uncle_fu -l Chinese --text "总之，魏玛共和国政府的不稳定往往被夸大了，政府的频繁更迭掩盖了某些部门长期保持的连续性。" -o uncle_fu_test.wav

stream测试方法, base服务启动后:
curl -sN http://localhost:8092/v1/tts/stream \
  -d '{"text":"总之，魏玛共和国政府的不稳定往往被夸大了，政府的频繁更迭掩盖了某些部门长期保持的连续性。"}' | \
  play -t raw -r 24000 -e signed -b 16 -c 1 -
  有音频播放, 但是agent无法听, 可以修改这个指令, 把输出保存在文件, 验证文件存在即可
