#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <unistd.h>
#endif

#include "guerrillamail_client.h"

enum {
    POLL_INTERVAL_SECONDS = 5,
    POLL_TIMEOUT_SECONDS = 120,
};

static int fail(gm_status_t status) {
    const char *message = gm_last_error_message();
    fprintf(stderr, "status=%d error=%s\n", (int)status, message ? message : "(none)");
    return 1;
}

static void sleep_seconds(unsigned int seconds) {
#ifdef _WIN32
    Sleep(seconds * 1000U);
#else
    sleep(seconds);
#endif
}

static void seed_random_once(void) {
    static bool seeded = false;

    if (!seeded) {
        srand((unsigned int)time(NULL));
        seeded = true;
    }
}

static void make_random_alias(char *buffer, size_t buffer_len) {
    unsigned int value;

    seed_random_once();
    value = ((unsigned int)rand()) % 100000U;
    snprintf(buffer, buffer_len, "demo%05u", value);
}

static void print_full_message(const gm_email_details_t *details) {
    printf("\nFull message\n");
    printf("============\n");
    printf("Message ID: %s\n", details->mail_id.ptr);
    printf("From: %s\n", details->mail_from.ptr);
    printf("Subject: %s\n", details->mail_subject.ptr);
    printf("Timestamp: %s\n", details->mail_timestamp.ptr);
    if (details->has_attachment_count) {
        printf("Attachments: %u\n", details->attachment_count);
    }
    printf("\nBody:\n%s\n", details->mail_body.ptr);
}

int main(void) {
    char alias[32];
    gm_client_t *client = NULL;
    gm_string_t email = {0};
    gm_message_list_t messages = {0};
    gm_email_details_t *details = NULL;
    gm_status_t status;
    bool deleted = false;
    int exit_code = 0;
    time_t deadline;

    status = gm_client_new_default(&client);
    if (status != GM_OK) {
        return fail(status);
    }

    make_random_alias(alias, sizeof(alias));
    status = gm_client_create_email(client, alias, &email);
    if (status != GM_OK) {
        exit_code = fail(status);
        goto cleanup;
    }

    printf("Temporary email: %s\n", email.ptr);
    printf("Send an email to that address. Polling every %d seconds for up to %d seconds.\n",
           POLL_INTERVAL_SECONDS,
           POLL_TIMEOUT_SECONDS);

    deadline = time(NULL) + POLL_TIMEOUT_SECONDS;
    for (;;) {
        unsigned int remaining_seconds;
        time_t now;

        status = gm_client_get_messages(client, email.ptr, &messages);
        if (status != GM_OK) {
            exit_code = fail(status);
            goto cleanup;
        }

        if (messages.len > 0) {
            printf("\nReceived %zu message(s). Fetching the first message...\n", messages.len);
            status =
                gm_client_fetch_email(client, email.ptr, messages.ptr[0].mail_id.ptr, &details);
            if (status != GM_OK) {
                exit_code = fail(status);
                goto cleanup;
            }

            print_full_message(details);
            break;
        }

        gm_message_list_free(&messages);

        now = time(NULL);
        if (now >= deadline) {
            fprintf(stderr, "Timed out waiting for a message.\n");
            exit_code = 1;
            goto cleanup;
        }

        remaining_seconds = (unsigned int)(deadline - now);
        printf("No messages yet. %u seconds remaining...\n", remaining_seconds);
        fflush(stdout);
        sleep_seconds(POLL_INTERVAL_SECONDS);
    }

cleanup:
    if (details != NULL) {
        gm_email_details_free(details);
    }
    gm_message_list_free(&messages);

    if (client != NULL && email.ptr != NULL) {
        status = gm_client_delete_email(client, email.ptr, &deleted);
        if (status == GM_OK) {
            printf("\nDeleted temporary email: %s\n", deleted ? "true" : "false");
        } else if (exit_code == 0) {
            exit_code = fail(status);
        } else {
            const char *message = gm_last_error_message();
            fprintf(stderr,
                    "cleanup status=%d error=%s\n",
                    (int)status,
                    message ? message : "(none)");
        }
    }

    gm_string_free(&email);
    gm_client_free(client);
    return exit_code;
}
