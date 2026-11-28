// Encore for Pebble (stage W2, docs/adr/android-0009-pebble.md; mock-up 50, screen 6).
//
// One window: the artist, the title in bold, a rule, the lyric line being sung in italics,
// and an action bar: up = previous, select = play or pause, down = next, and a long press
// of select = Like. The music plays on the phone; this is its remote. The phone's Encore
// sends Now Playing through the Pebble app as AppMessages (title, artist, lyric, playing,
// liked, private) and gets the buttons back as one "cmd" each.
//
// With "private" set (the phone's "Hide listening history while locked", while nothing
// plays), the phone sends no titles and this shows only "Encore" and the controls.
// The Pebble's fonts have no italic, so the lyric is slanted here: drawn upright, then
// each pixel row of each line is shifted right a little more the higher it is.

#include <pebble.h>

// The phone cuts each text to fit (pebble/PebbleProtocol.kt, MAX_*_BYTES), so these hold
// the longest it sends with the terminating zero.
#define TITLE_SIZE 101
#define ARTIST_SIZE 101
#define LYRIC_SIZE 161

// Three texts at their longest and four small numbers fit well inside 512 bytes.
#define INBOX_SIZE 512
#define OUTBOX_SIZE 64

#define HELLO_RETRY_MS 15000
#define LONG_PRESS_MS 700

// One pixel of slant for every SLANT rows up a line.
#define SLANT 4

// The "cmd" values, as the phone reads them (PebbleCommand in PebbleProtocol.kt).
enum {
  CMD_HELLO = 1,
  CMD_PREVIOUS = 2,
  CMD_PLAY_PAUSE = 3,
  CMD_NEXT = 4,
  CMD_LIKE = 5,
};

static Window *s_window;
static Layer *s_content;
static ActionBarLayer *s_action_bar;
static GBitmap *s_icon_prev;
static GBitmap *s_icon_play;
static GBitmap *s_icon_pause;
static GBitmap *s_icon_next;
static AppTimer *s_hello_timer;

static char s_title[TITLE_SIZE];
static char s_artist[ARTIST_SIZE];
static char s_lyric[LYRIC_SIZE];
static bool s_playing;
static bool s_liked;
static bool s_private;
// Whether the phone has answered since the app opened.
static bool s_heard;

// --- Drawing ---

static bool prv_big_screen(void) {
  return PBL_DISPLAY_WIDTH >= 200;
}

static GFont prv_font(const char *key) {
  return fonts_get_system_font(key);
}

// Where the text goes: clear of the action bar, and inside the circle on a round screen.
static GRect prv_text_area(GRect bounds) {
#if defined(PBL_ROUND)
  const int16_t side = bounds.size.w / 8;
  const int16_t top = bounds.size.h / 7;
  return GRect(side, top, bounds.size.w - side - ACTION_BAR_WIDTH - 6, bounds.size.h - 2 * top);
#else
  return GRect(5, 2, bounds.size.w - ACTION_BAR_WIDTH - 9, bounds.size.h - 6);
#endif
}

static GTextAlignment prv_alignment(void) {
  return PBL_IF_ROUND_ELSE(GTextAlignmentCenter, GTextAlignmentLeft);
}

// Draws text in box and returns the height it took.
static int16_t prv_draw(GContext *ctx, const char *text, GFont font, GRect box) {
  const GSize size = graphics_text_layout_get_content_size(
      text, font, box, GTextOverflowModeTrailingEllipsis, prv_alignment());
  graphics_draw_text(ctx, text, font, box, GTextOverflowModeTrailingEllipsis, prv_alignment(),
                     NULL);
  return size.h;
}

static void prv_draw_centred(GContext *ctx, GRect area, const char *text, GFont font) {
  const GSize size = graphics_text_layout_get_content_size(
      text, font, area, GTextOverflowModeWordWrap, GTextAlignmentCenter);
  GRect box = GRect(area.origin.x, area.origin.y + (area.size.h - size.h) / 2 - 4, area.size.w,
                    size.h + 8);
  graphics_draw_text(ctx, text, font, box, GTextOverflowModeWordWrap, GTextAlignmentCenter, NULL);
}

// A small filled heart centred on (cx, cy): the song is liked.
static void prv_draw_heart(GContext *ctx, int16_t cx, int16_t cy) {
  graphics_context_set_fill_color(ctx, COLOR_FALLBACK(GColorFolly, GColorBlack));
  graphics_context_set_stroke_color(ctx, COLOR_FALLBACK(GColorFolly, GColorBlack));
  graphics_fill_circle(ctx, GPoint(cx - 3, cy - 2), 3);
  graphics_fill_circle(ctx, GPoint(cx + 3, cy - 2), 3);
  for (int16_t i = 0; i <= 6; i++) {
    graphics_draw_line(ctx, GPoint(cx - 6 + i, cy - 1 + i), GPoint(cx + 6 - i, cy - 1 + i));
  }
}

static uint8_t prv_get_pixel(const GBitmapDataRowInfo *row, bool one_bit, int16_t x) {
  if (one_bit) {
    return (row->data[x >> 3] >> (x & 7)) & 1;
  }
  return row->data[x];
}

static void prv_set_pixel(const GBitmapDataRowInfo *row, bool one_bit, int16_t x, uint8_t v) {
  if (one_bit) {
    const uint8_t mask = (uint8_t)(1 << (x & 7));
    if (v) {
      row->data[x >> 3] |= mask;
    } else {
      row->data[x >> 3] &= (uint8_t)~mask;
    }
  } else {
    row->data[x] = v;
  }
}

// Slants what was drawn in box (screen coordinates): each row of each line_h-high line moves
// right by its height above the line's foot over SLANT. The text was drawn narrower than box
// by the largest shift, so nothing is pushed out.
static void prv_slant(GContext *ctx, GRect box, int16_t line_h) {
  if (line_h <= SLANT) {
    return;
  }
  GBitmap *fb = graphics_capture_frame_buffer(ctx);
  if (!fb) {
    return;
  }
  const GBitmapFormat format = gbitmap_get_format(fb);
  if (format == GBitmapFormat1Bit || format == GBitmapFormat8Bit ||
      format == GBitmapFormat8BitCircular) {
    const bool one_bit = format == GBitmapFormat1Bit;
    // The window's white: 1 in a 1-bit frame buffer, GColorWhite's byte in an 8-bit one.
    const uint8_t white = one_bit ? 1 : GColorWhite.argb;
    const GRect fb_bounds = gbitmap_get_bounds(fb);
    const int16_t top = box.origin.y < fb_bounds.origin.y ? fb_bounds.origin.y : box.origin.y;
    int16_t bottom = box.origin.y + box.size.h;
    if (bottom > fb_bounds.origin.y + fb_bounds.size.h) {
      bottom = fb_bounds.origin.y + fb_bounds.size.h;
    }
    for (int16_t y = top; y < bottom; y++) {
      const int16_t in_line = (y - box.origin.y) % line_h;
      const int16_t shift = (line_h - 1 - in_line) / SLANT;
      if (shift <= 0) {
        continue;
      }
      const GBitmapDataRowInfo row = gbitmap_get_data_row_info(fb, y);
      const int16_t x0 = box.origin.x > row.min_x ? box.origin.x : row.min_x;
      int16_t x1 = box.origin.x + box.size.w - 1;
      if (x1 > row.max_x) {
        x1 = row.max_x;
      }
      if (x1 - x0 <= shift) {
        continue;
      }
      for (int16_t x = x1; x >= x0 + shift; x--) {
        prv_set_pixel(&row, one_bit, x, prv_get_pixel(&row, one_bit, x - shift));
      }
      for (int16_t x = x0; x < x0 + shift; x++) {
        prv_set_pixel(&row, one_bit, x, white);
      }
    }
  }
  graphics_release_frame_buffer(ctx, fb);
}

// The distance between two wrapped lines of font, measured, so the slant follows the lines.
static int16_t prv_line_height(GFont font, int16_t width) {
  const GRect box = GRect(0, 0, width, 200);
  const int16_t one = graphics_text_layout_get_content_size(
      "Ag", font, box, GTextOverflowModeWordWrap, GTextAlignmentLeft).h;
  const int16_t two = graphics_text_layout_get_content_size(
      "Ag\nAg", font, box, GTextOverflowModeWordWrap, GTextAlignmentLeft).h;
  return two > one ? two - one : one;
}

static void prv_content_update(Layer *layer, GContext *ctx) {
  const GRect bounds = layer_get_bounds(layer);
  graphics_context_set_fill_color(ctx, GColorWhite);
  graphics_fill_rect(ctx, bounds, 0, GCornerNone);
  graphics_context_set_text_color(ctx, GColorBlack);

  const bool big = prv_big_screen();
  const GFont small = prv_font(big ? FONT_KEY_GOTHIC_24 : FONT_KEY_GOTHIC_18);
  const GFont bold = prv_font(big ? FONT_KEY_GOTHIC_28_BOLD : FONT_KEY_GOTHIC_24_BOLD);
  const GRect area = prv_text_area(bounds);

  if (s_private) {
    prv_draw_centred(ctx, area, "Encore", bold);
    return;
  }
  if (!s_heard || s_title[0] == '\0') {
    prv_draw_centred(ctx, area, "Open Encore on your phone", small);
    return;
  }

  const int16_t small_line = big ? 30 : 22;
  const int16_t bold_lines = big ? 68 : 56;
  int16_t y = area.origin.y;

  // The artist, with room for the heart when the song is liked.
  const int16_t heart_room = s_liked ? 16 : 0;
  y += prv_draw(ctx, s_artist, small,
                GRect(area.origin.x, y, area.size.w - heart_room, small_line));
  if (s_liked) {
    prv_draw_heart(ctx, area.origin.x + area.size.w - 7, area.origin.y + small_line / 2);
  }
  y += 2;

  // The title in bold, two lines at most.
  y += prv_draw(ctx, s_title, bold, GRect(area.origin.x, y, area.size.w, bold_lines));
  y += 6;

  if (s_lyric[0] == '\0') {
    return;
  }
  // The rule, then the line being sung.
  graphics_context_set_fill_color(ctx, COLOR_FALLBACK(GColorImperialPurple, GColorBlack));
  graphics_fill_rect(ctx, GRect(area.origin.x, y, area.size.w, 2), 0, GCornerNone);
  y += 4;

  const int16_t bottom = area.origin.y + area.size.h;
  if (bottom - y < small_line) {
    return;
  }
  const int16_t line_h = prv_line_height(small, area.size.w);
  const int16_t max_shift = line_h / SLANT;
  const GRect lyric_box = GRect(area.origin.x, y, area.size.w, bottom - y);
  graphics_context_set_text_color(ctx, COLOR_FALLBACK(GColorImperialPurple, GColorBlack));
  graphics_draw_text(ctx, s_lyric, small,
                     GRect(lyric_box.origin.x, lyric_box.origin.y, lyric_box.size.w - max_shift,
                           lyric_box.size.h),
                     GTextOverflowModeTrailingEllipsis, prv_alignment(), NULL);
  // The content layer sits at the window's top left, so its coordinates are the screen's.
  const GRect frame = layer_get_frame(layer);
  prv_slant(ctx, GRect(frame.origin.x + lyric_box.origin.x, frame.origin.y + lyric_box.origin.y,
                       lyric_box.size.w, lyric_box.size.h),
            line_h);
}

static void prv_update_icons(void) {
  action_bar_layer_set_icon_animated(s_action_bar, BUTTON_ID_SELECT,
                                     s_playing ? s_icon_pause : s_icon_play, true);
}

// --- Messages ---

static void prv_send(uint8_t cmd) {
  DictionaryIterator *iter;
  if (app_message_outbox_begin(&iter) != APP_MSG_OK || !iter) {
    return;
  }
  dict_write_uint8(iter, MESSAGE_KEY_cmd, cmd);
  app_message_outbox_send();
}

static void prv_hello(void *data) {
  s_hello_timer = NULL;
  if (s_heard) {
    return;
  }
  prv_send(CMD_HELLO);
  // Until the phone answers: Encore may not be running yet.
  s_hello_timer = app_timer_register(HELLO_RETRY_MS, prv_hello, NULL);
}

static void prv_copy_text(char *out, size_t size, const Tuple *t) {
  if (!t || t->type != TUPLE_CSTRING) {
    out[0] = '\0';
    return;
  }
  size_t length = t->length;
  if (length >= size) {
    length = size - 1;
  }
  memcpy(out, t->value->cstring, length);
  out[length] = '\0';
}

static bool prv_read_flag(const Tuple *t) {
  if (!t) {
    return false;
  }
  if (t->type == TUPLE_UINT || t->type == TUPLE_INT) {
    switch (t->length) {
      case 1:
        return t->value->uint8 != 0;
      case 2:
        return t->value->uint16 != 0;
      case 4:
        return t->value->uint32 != 0;
      default:
        return false;
    }
  }
  return false;
}

static void prv_inbox_received(DictionaryIterator *iter, void *context) {
  // The phone always sends the whole state, so a missing key is an empty one.
  prv_copy_text(s_title, sizeof(s_title), dict_find(iter, MESSAGE_KEY_title));
  prv_copy_text(s_artist, sizeof(s_artist), dict_find(iter, MESSAGE_KEY_artist));
  prv_copy_text(s_lyric, sizeof(s_lyric), dict_find(iter, MESSAGE_KEY_lyric));
  s_playing = prv_read_flag(dict_find(iter, MESSAGE_KEY_playing));
  s_liked = prv_read_flag(dict_find(iter, MESSAGE_KEY_liked));
  // Missing means private: the stricter reading, as on the Wear OS watch.
  const Tuple *private_flag = dict_find(iter, MESSAGE_KEY_private);
  s_private = private_flag ? prv_read_flag(private_flag) : true;
  if (s_private) {
    s_title[0] = '\0';
    s_artist[0] = '\0';
    s_lyric[0] = '\0';
  }
  s_heard = true;
  if (s_hello_timer) {
    app_timer_cancel(s_hello_timer);
    s_hello_timer = NULL;
  }
  prv_update_icons();
  layer_mark_dirty(s_content);
}

static void prv_inbox_dropped(AppMessageResult reason, void *context) {
  APP_LOG(APP_LOG_LEVEL_WARNING, "Dropped a message: %d", (int)reason);
}

static void prv_outbox_failed(DictionaryIterator *iter, AppMessageResult reason, void *context) {
  APP_LOG(APP_LOG_LEVEL_INFO, "The phone didn't take a message: %d", (int)reason);
}

// --- Buttons ---

static void prv_up_click(ClickRecognizerRef recognizer, void *context) {
  prv_send(CMD_PREVIOUS);
}

static void prv_select_click(ClickRecognizerRef recognizer, void *context) {
  prv_send(CMD_PLAY_PAUSE);
  // Shown at once; the phone's answer puts it right if the press didn't take.
  s_playing = !s_playing;
  prv_update_icons();
}

static void prv_down_click(ClickRecognizerRef recognizer, void *context) {
  prv_send(CMD_NEXT);
}

// Like or unlike the song playing (the phone decides which, from what it knows).
static void prv_like_click(ClickRecognizerRef recognizer, void *context) {
  prv_send(CMD_LIKE);
  vibes_short_pulse();
  if (!s_private && s_title[0] != '\0') {
    s_liked = !s_liked;
    layer_mark_dirty(s_content);
  }
}

static void prv_click_config(void *context) {
  window_single_click_subscribe(BUTTON_ID_UP, prv_up_click);
  window_single_click_subscribe(BUTTON_ID_SELECT, prv_select_click);
  window_single_click_subscribe(BUTTON_ID_DOWN, prv_down_click);
  window_long_click_subscribe(BUTTON_ID_SELECT, LONG_PRESS_MS, prv_like_click, NULL);
}

// --- The window ---

static void prv_window_load(Window *window) {
  Layer *root = window_get_root_layer(window);
  const GRect bounds = layer_get_bounds(root);

  s_content = layer_create(bounds);
  layer_set_update_proc(s_content, prv_content_update);
  layer_add_child(root, s_content);

  s_icon_prev = gbitmap_create_with_resource(RESOURCE_ID_ICON_PREV);
  s_icon_play = gbitmap_create_with_resource(RESOURCE_ID_ICON_PLAY);
  s_icon_pause = gbitmap_create_with_resource(RESOURCE_ID_ICON_PAUSE);
  s_icon_next = gbitmap_create_with_resource(RESOURCE_ID_ICON_NEXT);

  s_action_bar = action_bar_layer_create();
  action_bar_layer_set_background_color(s_action_bar, GColorBlack);
  action_bar_layer_set_click_config_provider(s_action_bar, prv_click_config);
  action_bar_layer_set_icon(s_action_bar, BUTTON_ID_UP, s_icon_prev);
  action_bar_layer_set_icon(s_action_bar, BUTTON_ID_SELECT, s_icon_play);
  action_bar_layer_set_icon(s_action_bar, BUTTON_ID_DOWN, s_icon_next);
  action_bar_layer_add_to_window(s_action_bar, window);
}

static void prv_window_unload(Window *window) {
  action_bar_layer_destroy(s_action_bar);
  layer_destroy(s_content);
  gbitmap_destroy(s_icon_prev);
  gbitmap_destroy(s_icon_play);
  gbitmap_destroy(s_icon_pause);
  gbitmap_destroy(s_icon_next);
}

static void prv_init(void) {
  s_window = window_create();
  window_set_background_color(s_window, GColorWhite);
  window_set_window_handlers(s_window, (WindowHandlers){
                                           .load = prv_window_load,
                                           .unload = prv_window_unload,
                                       });
  window_stack_push(s_window, true);

  app_message_register_inbox_received(prv_inbox_received);
  app_message_register_inbox_dropped(prv_inbox_dropped);
  app_message_register_outbox_failed(prv_outbox_failed);
  app_message_open(INBOX_SIZE, OUTBOX_SIZE);

  // Tell the phone the app is open: it sends Now Playing only while it is.
  prv_hello(NULL);
}

static void prv_deinit(void) {
  if (s_hello_timer) {
    app_timer_cancel(s_hello_timer);
  }
  window_destroy(s_window);
}

int main(void) {
  prv_init();
  app_event_loop();
  prv_deinit();
}
