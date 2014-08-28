// Copyright 2014 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#ifndef __HTML5EVER_H
#define __HTML5EVER_H

#include <stdlib.h>

struct h5e_buf {
    unsigned char *data;
    size_t len;
};

struct h5e_buf h5e_buf_from_cstr(const char *str);

struct h5e_token_ops {
    void (*do_doctype)(void *user, struct h5e_buf name,
        struct h5e_buf pub, struct h5e_buf sys, int force_quirks);
    void (*do_start_tag)(void *user, struct h5e_buf name,
        int self_closing, size_t num_attrs);
    void (*do_tag_attr)(void *user, struct h5e_buf name, struct h5e_buf value);
    void (*do_end_tag)(void *user, struct h5e_buf name);
    void (*do_comment)(void *user, struct h5e_buf text);
    void (*do_chars)(void *user, struct h5e_buf text);
    void (*do_null_char)(void *user);
    void (*do_eof)(void *user);
    void (*do_error)(void *user, struct h5e_buf message);
};

struct h5e_token_sink {
    struct h5e_token_ops *ops;
    void *user;
};

struct h5e_tokenizer;

struct h5e_tokenizer *h5e_tokenizer_new(struct h5e_token_sink *sink);
void h5e_tokenizer_free(struct h5e_tokenizer *tok);
void h5e_tokenizer_feed(struct h5e_tokenizer *tok, struct h5e_buf buf);
void h5e_tokenizer_end(struct h5e_tokenizer *tok);

#endif
