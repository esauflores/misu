/* A real PipeWire consumer for portal-check.py; require nonuniform screen pixels. */
#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <png.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct capture {
    struct pw_main_loop *loop;
    struct pw_stream *stream;
    struct spa_video_info_raw info;
    const char *file;
    unsigned good, wanted;
};

static int save_png(struct capture *c, const unsigned char *pixels, int stride) {
    FILE *file = fopen(c->file, "wb");
    if (!file) return -1;
    png_structp png = png_create_write_struct(PNG_LIBPNG_VER_STRING, NULL, NULL, NULL);
    png_infop info = png ? png_create_info_struct(png) : NULL;
    if (!info || setjmp(png_jmpbuf(png))) {
        if (png) png_destroy_write_struct(&png, info ? &info : NULL);
        fclose(file);
        return -1;
    }
    png_init_io(png, file);
    png_set_IHDR(png, info, c->info.size.width, c->info.size.height, 8,
                 PNG_COLOR_TYPE_RGBA, PNG_INTERLACE_NONE, PNG_COMPRESSION_TYPE_DEFAULT,
                 PNG_FILTER_TYPE_DEFAULT);
    png_set_bgr(png);
    png_write_info(png, info);
    for (unsigned y = 0; y < c->info.size.height; y++)
        png_write_row(png, pixels + (size_t)y * stride);
    png_write_end(png, info);
    png_destroy_write_struct(&png, &info);
    fclose(file);
    return 0;
}

static void changed(void *data, uint32_t id, const struct spa_pod *param) {
    struct capture *c = data;
    if (id == SPA_PARAM_Format && param) spa_format_video_raw_parse(param, &c->info);
}

static void state_changed(void *data, enum pw_stream_state old,
                          enum pw_stream_state state, const char *error) {
    (void)old;
    struct capture *c = data;
    if (state == PW_STREAM_STATE_ERROR) {
        fprintf(stderr, "PipeWire: %s\n", error ? error : "stream failed");
        pw_main_loop_quit(c->loop);
    }
}

static void process(void *data) {
    struct capture *c = data;
    struct pw_buffer *buffer = pw_stream_dequeue_buffer(c->stream);
    if (!buffer) return;
    if (buffer->buffer->n_datas) {
        struct spa_data *d = &buffer->buffer->datas[0];
        unsigned width = c->info.size.width, height = c->info.size.height;
        if (d->data && d->chunk && width && height && c->info.format == SPA_VIDEO_FORMAT_BGRA
            && d->chunk->stride >= (int64_t)width * 4
            && d->chunk->offset + (uint64_t)(height - 1) * d->chunk->stride + width * 4 <= d->maxsize) {
            const unsigned char *pixels = (unsigned char *)d->data + d->chunk->offset;
            int varied = 0;
            for (unsigned y = 0; y < height && !varied; y++)
                for (unsigned x = 0; x < width && !varied; x++)
                    varied = memcmp(pixels, pixels + (size_t)y * d->chunk->stride + x * 4, 3) != 0;
            if (varied && save_png(c, pixels, d->chunk->stride) == 0) c->good++;
        }
    }
    pw_stream_queue_buffer(c->stream, buffer);
    if (c->good >= c->wanted) pw_main_loop_quit(c->loop);
}

static const struct pw_stream_events events = {
    PW_VERSION_STREAM_EVENTS, .state_changed = state_changed, .param_changed = changed, .process = process
};

int main(int argc, char **argv) {
    if (argc != 5 || atoi(argv[4]) < 1) return 2;
    pw_init(NULL, NULL);
    struct capture c = {.file = argv[3], .wanted = (unsigned)atoi(argv[4])};
    c.loop = pw_main_loop_new(NULL);
    struct pw_context *ctx = pw_context_new(pw_main_loop_get_loop(c.loop), NULL, 0);
    struct pw_core *core = pw_context_connect_fd(ctx, atoi(argv[1]), NULL, 0);
    if (!core) return 3;
    c.stream = pw_stream_new(core, "misu-capture-check", pw_properties_new(
        PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY, "Capture", PW_KEY_MEDIA_ROLE, "Screen", NULL));
    struct spa_hook listener;
    pw_stream_add_listener(c.stream, &listener, &events, &c);
    unsigned char bytes[1024];
    struct spa_pod_builder builder = SPA_POD_BUILDER_INIT(bytes, sizeof(bytes));
    const struct spa_pod *params[] = {spa_format_video_raw_build(&builder, SPA_PARAM_EnumFormat,
        &(struct spa_video_info_raw){.format = SPA_VIDEO_FORMAT_BGRA, .framerate = {0, 1}})};
    if (pw_stream_connect(c.stream, PW_DIRECTION_INPUT, atoi(argv[2]),
        PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS, params, 1) < 0) return 4;
    pw_main_loop_run(c.loop);
    pw_stream_destroy(c.stream);
    pw_core_disconnect(core);
    pw_context_destroy(ctx);
    pw_main_loop_destroy(c.loop);
    pw_deinit();
    printf("Received %u nonuniform screen frame(s)\n", c.good);
    return c.good >= c.wanted ? 0 : 1;
}
