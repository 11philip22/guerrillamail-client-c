#include <stdbool.h>
#include <stdio.h>

#include "guerrillamail_client.h"

static int fail(gm_status_t status) {
    const char *message = gm_last_error_message();
    fprintf(stderr, "status=%d error=%s\n", (int)status, message ? message : "(none)");
    return 1;
}

int main(void) {
    gm_client_t *client = NULL;
    gm_status_t status = gm_client_new_default(&client);
    if (status != GM_OK) {
        return fail(status);
    }

    gm_string_t email = {0};
    status = gm_client_create_email(client, "demoalias", &email);
    if (status != GM_OK) {
        gm_client_free(client);
        return fail(status);
    }

    printf("email: %s\n", email.ptr);

    gm_message_list_t messages = {0};
    status = gm_client_get_messages(client, email.ptr, &messages);
    if (status != GM_OK) {
        gm_string_free(&email);
        gm_client_free(client);
        return fail(status);
    }

    printf("messages: %zu\n", messages.len);
    if (messages.len > 0) {
        gm_email_details_t *details = NULL;
        status = gm_client_fetch_email(client, email.ptr, messages.ptr[0].mail_id.ptr, &details);
        if (status != GM_OK) {
            gm_message_list_free(&messages);
            gm_string_free(&email);
            gm_client_free(client);
            return fail(status);
        }

        printf("subject: %s\n", details->mail_subject.ptr);
        gm_email_details_free(details);
    }

    bool deleted = false;
    status = gm_client_delete_email(client, email.ptr, &deleted);
    if (status != GM_OK) {
        gm_message_list_free(&messages);
        gm_string_free(&email);
        gm_client_free(client);
        return fail(status);
    }

    printf("deleted: %s\n", deleted ? "true" : "false");

    gm_message_list_free(&messages);
    gm_string_free(&email);
    gm_client_free(client);
    return 0;
}

