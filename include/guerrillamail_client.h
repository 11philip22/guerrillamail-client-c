#ifndef GUERRILLAMAIL_CLIENT_H
#define GUERRILLAMAIL_CLIENT_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct gm_builder_t gm_builder_t;
typedef struct gm_client_t gm_client_t;

typedef enum gm_status_t {
    GM_OK = 0,
    GM_ERR_NULL = 1,
    GM_ERR_INVALID_ARGUMENT = 2,
    GM_ERR_REQUEST = 3,
    GM_ERR_RESPONSE_PARSE = 4,
    GM_ERR_TOKEN_PARSE = 5,
    GM_ERR_JSON = 6,
    GM_ERR_INTERNAL = 7
} gm_status_t;

typedef struct gm_string_t {
    char *ptr;
    size_t len;
} gm_string_t;

typedef struct gm_message_t {
    gm_string_t mail_id;
    gm_string_t mail_from;
    gm_string_t mail_subject;
    gm_string_t mail_excerpt;
    gm_string_t mail_timestamp;
} gm_message_t;

typedef struct gm_message_list_t {
    gm_message_t *ptr;
    size_t len;
} gm_message_list_t;

typedef struct gm_email_details_t {
    gm_string_t mail_id;
    gm_string_t mail_from;
    gm_string_t mail_subject;
    gm_string_t mail_body;
    gm_string_t mail_timestamp;
    uint32_t attachment_count;
    bool has_attachment_count;
} gm_email_details_t;

gm_status_t gm_builder_new(gm_builder_t **out_builder);
void gm_builder_free(gm_builder_t *builder);
gm_status_t gm_builder_set_proxy(gm_builder_t *builder, const char *proxy);
gm_status_t gm_builder_set_danger_accept_invalid_certs(gm_builder_t *builder, bool value);
gm_status_t gm_builder_set_user_agent(gm_builder_t *builder, const char *user_agent);
gm_status_t gm_builder_set_timeout_ms(gm_builder_t *builder, uint64_t timeout_ms);
gm_status_t gm_builder_build(gm_builder_t *builder, gm_client_t **out_client);

gm_status_t gm_client_new_default(gm_client_t **out_client);
void gm_client_free(gm_client_t *client);
gm_status_t gm_client_create_email(
    gm_client_t *client,
    const char *alias,
    gm_string_t *out_email
);
gm_status_t gm_client_get_messages(
    gm_client_t *client,
    const char *email,
    gm_message_list_t *out_messages
);
gm_status_t gm_client_fetch_email(
    gm_client_t *client,
    const char *email,
    const char *mail_id,
    gm_email_details_t **out_details
);
gm_status_t gm_client_delete_email(
    gm_client_t *client,
    const char *email,
    bool *out_deleted
);

void gm_string_free(gm_string_t *value);
void gm_message_list_free(gm_message_list_t *messages);
void gm_email_details_free(gm_email_details_t *details);

const char *gm_last_error_message(void);
void gm_last_error_clear(void);

#ifdef __cplusplus
}
#endif

#endif

