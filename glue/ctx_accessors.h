#ifndef QWEN_TTS_CTX_ACCESSORS_H
#define QWEN_TTS_CTX_ACCESSORS_H

#include "qwen_tts.h"

#ifdef __cplusplus
extern "C" {
#endif

int qwen_ctx_get_temperature(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_temperature(qwen_tts_ctx_t *ctx, float v);
int qwen_ctx_get_top_k(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_top_k(qwen_tts_ctx_t *ctx, int v);
float qwen_ctx_get_top_p(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_top_p(qwen_tts_ctx_t *ctx, float v);
float qwen_ctx_get_rep_penalty(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_rep_penalty(qwen_tts_ctx_t *ctx, float v);
int qwen_ctx_get_max_tokens(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_max_tokens(qwen_tts_ctx_t *ctx, int v);
int qwen_ctx_get_speaker_id(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_speaker_id(qwen_tts_ctx_t *ctx, int v);
int qwen_ctx_get_language_id(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_language_id(qwen_tts_ctx_t *ctx, int v);

const char *qwen_ctx_get_instruct(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_instruct(qwen_tts_ctx_t *ctx, const char *s);
void qwen_ctx_free_instruct(qwen_tts_ctx_t *ctx);

int qwen_ctx_get_voice_design(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_voice_design(qwen_tts_ctx_t *ctx, int v);
int qwen_ctx_get_voice_clone(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_voice_clone(qwen_tts_ctx_t *ctx, int v);
int qwen_ctx_get_xvector_only(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_xvector_only(qwen_tts_ctx_t *ctx, int v);

const char *qwen_ctx_get_ref_audio_path(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_ref_audio_path(qwen_tts_ctx_t *ctx, const char *s);
void qwen_ctx_free_ref_audio_path(qwen_tts_ctx_t *ctx);

const char *qwen_ctx_get_ref_text(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_ref_text(qwen_tts_ctx_t *ctx, const char *s);
void qwen_ctx_free_ref_text(qwen_tts_ctx_t *ctx);

int qwen_ctx_get_is_base_model(qwen_tts_ctx_t *ctx);
int qwen_ctx_get_use_int8(qwen_tts_ctx_t *ctx);
int qwen_ctx_get_use_int4(qwen_tts_ctx_t *ctx);

int qwen_ctx_get_stream(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_stream(qwen_tts_ctx_t *ctx, int v);
int qwen_ctx_get_stream_chunk_frames(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_stream_chunk_frames(qwen_tts_ctx_t *ctx, int v);

uint32_t qwen_ctx_get_seed(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_seed(qwen_tts_ctx_t *ctx, uint32_t v);

float qwen_ctx_get_cp_roughness(qwen_tts_ctx_t *ctx);
void qwen_ctx_set_cp_roughness(qwen_tts_ctx_t *ctx, float v);

void qwen_ctx_reset_state(qwen_tts_ctx_t *ctx);

int qwen_ctx_init_voice_clone(qwen_tts_ctx_t *ctx, const char *ref_audio_path, const char *ref_text,
                               int xvector_only, int silent);

void qwen_ctx_clear_audio_callback(qwen_tts_ctx_t *ctx);

#ifdef __cplusplus
}
#endif

#endif /* QWEN_TTS_CTX_ACCESSORS_H */
