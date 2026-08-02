#include "ctx_accessors.h"
#include <stdlib.h>
#include <string.h>
#include <time.h>

int qwen_ctx_get_temperature(qwen_tts_ctx_t *ctx) { return ctx->temperature; }
void qwen_ctx_set_temperature(qwen_tts_ctx_t *ctx, float v) { ctx->temperature = v; }

int qwen_ctx_get_top_k(qwen_tts_ctx_t *ctx) { return ctx->top_k; }
void qwen_ctx_set_top_k(qwen_tts_ctx_t *ctx, int v) { ctx->top_k = v; }

float qwen_ctx_get_top_p(qwen_tts_ctx_t *ctx) { return ctx->top_p; }
void qwen_ctx_set_top_p(qwen_tts_ctx_t *ctx, float v) { ctx->top_p = v; }

float qwen_ctx_get_rep_penalty(qwen_tts_ctx_t *ctx) { return ctx->rep_penalty; }
void qwen_ctx_set_rep_penalty(qwen_tts_ctx_t *ctx, float v) { ctx->rep_penalty = v; }

int qwen_ctx_get_max_tokens(qwen_tts_ctx_t *ctx) { return ctx->max_tokens; }
void qwen_ctx_set_max_tokens(qwen_tts_ctx_t *ctx, int v) { ctx->max_tokens = v; }

int qwen_ctx_get_speaker_id(qwen_tts_ctx_t *ctx) { return ctx->speaker_id; }
void qwen_ctx_set_speaker_id(qwen_tts_ctx_t *ctx, int v) { ctx->speaker_id = v; }

int qwen_ctx_get_language_id(qwen_tts_ctx_t *ctx) { return ctx->language_id; }
void qwen_ctx_set_language_id(qwen_tts_ctx_t *ctx, int v) { ctx->language_id = v; }

const char *qwen_ctx_get_instruct(qwen_tts_ctx_t *ctx) { return ctx->instruct; }
void qwen_ctx_set_instruct(qwen_tts_ctx_t *ctx, const char *s) {
    if (ctx->instruct) free(ctx->instruct);
    ctx->instruct = s ? strdup(s) : NULL;
}
void qwen_ctx_free_instruct(qwen_tts_ctx_t *ctx) {
    if (ctx->instruct) { free(ctx->instruct); ctx->instruct = NULL; }
}

int qwen_ctx_get_voice_design(qwen_tts_ctx_t *ctx) { return ctx->voice_design; }
void qwen_ctx_set_voice_design(qwen_tts_ctx_t *ctx, int v) { ctx->voice_design = v; }

int qwen_ctx_get_voice_clone(qwen_tts_ctx_t *ctx) { return ctx->voice_clone; }
void qwen_ctx_set_voice_clone(qwen_tts_ctx_t *ctx, int v) { ctx->voice_clone = v; }

int qwen_ctx_get_xvector_only(qwen_tts_ctx_t *ctx) { return ctx->xvector_only; }
void qwen_ctx_set_xvector_only(qwen_tts_ctx_t *ctx, int v) { ctx->xvector_only = v; }

const char *qwen_ctx_get_ref_audio_path(qwen_tts_ctx_t *ctx) { return ctx->ref_audio_path; }
void qwen_ctx_set_ref_audio_path(qwen_tts_ctx_t *ctx, const char *s) {
    if (ctx->ref_audio_path) free(ctx->ref_audio_path);
    ctx->ref_audio_path = s ? strdup(s) : NULL;
}
void qwen_ctx_free_ref_audio_path(qwen_tts_ctx_t *ctx) {
    if (ctx->ref_audio_path) { free(ctx->ref_audio_path); ctx->ref_audio_path = NULL; }
}

const char *qwen_ctx_get_ref_text(qwen_tts_ctx_t *ctx) { return ctx->ref_text; }
void qwen_ctx_set_ref_text(qwen_tts_ctx_t *ctx, const char *s) {
    if (ctx->ref_text) free(ctx->ref_text);
    ctx->ref_text = s ? strdup(s) : NULL;
}
void qwen_ctx_free_ref_text(qwen_tts_ctx_t *ctx) {
    if (ctx->ref_text) { free(ctx->ref_text); ctx->ref_text = NULL; }
}

int qwen_ctx_get_is_base_model(qwen_tts_ctx_t *ctx) { return ctx->is_base_model; }
int qwen_ctx_get_use_int8(qwen_tts_ctx_t *ctx) { return ctx->use_int8; }
int qwen_ctx_get_use_int4(qwen_tts_ctx_t *ctx) { return ctx->use_int4; }

int qwen_ctx_get_stream(qwen_tts_ctx_t *ctx) { return ctx->stream; }
void qwen_ctx_set_stream(qwen_tts_ctx_t *ctx, int v) { ctx->stream = v; }
int qwen_ctx_get_stream_chunk_frames(qwen_tts_ctx_t *ctx) { return ctx->stream_chunk_frames; }
void qwen_ctx_set_stream_chunk_frames(qwen_tts_ctx_t *ctx, int v) { ctx->stream_chunk_frames = v; }

uint32_t qwen_ctx_get_seed(qwen_tts_ctx_t *ctx) { return ctx->seed; }
void qwen_ctx_set_seed(qwen_tts_ctx_t *ctx, uint32_t v) { ctx->seed = v; }

float qwen_ctx_get_cp_roughness(qwen_tts_ctx_t *ctx) { return ctx->cp_roughness; }
void qwen_ctx_set_cp_roughness(qwen_tts_ctx_t *ctx, float v) { ctx->cp_roughness = v; }

void qwen_ctx_reset_state(qwen_tts_ctx_t *ctx) {
    int already_voice_clone = ctx->voice_clone;

    if (!already_voice_clone) {
        ctx->speaker_id = 3061;
        ctx->language_id = 2050;
    }

    ctx->temperature = 0.5;
    ctx->top_k = 50;
    ctx->top_p = 1.0;
    ctx->rep_penalty = 1.05;
    ctx->max_tokens = 8192;
    ctx->voice_design = 0;
    ctx->cp_roughness = 0.0;

    if (ctx->instruct) {
        free(ctx->instruct);
        ctx->instruct = NULL;
    }

    if (!ctx->seed) {
        ctx->seed = (uint32_t)time(NULL);
    }
}

int qwen_ctx_init_voice_clone(qwen_tts_ctx_t *ctx, const char *ref_audio_path, const char *ref_text,
                               int xvector_only, int silent) {
    int enc_dim = ctx->speaker_enc.enc_dim > 0 ? ctx->speaker_enc.enc_dim : ctx->config.hidden_size;

    ctx->voice_clone = 1;
    ctx->xvector_only = xvector_only ? 1 : (ref_text ? 0 : 1);
    if (ref_audio_path) {
        if (ctx->ref_audio_path) free(ctx->ref_audio_path);
        ctx->ref_audio_path = strdup(ref_audio_path);
    }
    if (ref_text) {
        if (ctx->ref_text) free(ctx->ref_text);
        ctx->ref_text = strdup(ref_text);
    }

    ctx->speaker_embedding = (float *)malloc(enc_dim * sizeof(float));
    if (!ctx->speaker_embedding) {
        return -1;
    }

    if (qwen_speech_encoder_load(ctx) != 0) {
        if (!silent) fprintf(stderr, "Warning: speech encoder load failed\n");
        return -1;
    }

    if (qwen_extract_speaker_embedding(ctx, ref_audio_path, ctx->speaker_embedding) != 0) {
        if (!silent) fprintf(stderr, "Warning: speaker embedding extraction failed\n");
        return -1;
    }

    return 0;
}

void qwen_ctx_clear_audio_callback(qwen_tts_ctx_t *ctx) {
    ctx->audio_cb = NULL;
    ctx->audio_cb_userdata = NULL;
}
