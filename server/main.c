#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mongoose.h"

#define JSON_HEADERS "Content-Type: application/json\r\n"
#define AUTH_LOG_MAX_SIZE (10 * 1024 * 1024)
#define AUTH_LOG_FILE "auth_audit.log"
#define AUTH_LOG_BACKUP "auth_audit.log.1"
#define AUTH_TICKET_LIFETIME_SECONDS (30 * 24 * 60 * 60)
#define AUTH_KEY_FILE "auth_keys.txt"
#define AUTH_PRIVATE_KEY_PREFIX "RIDEX_AUTH_PRIVATE_KEY="

struct auth_request
{
  char device_id[44];
  char nonce[44];
  char app_version[32];
  char platform[16];
};

static void write_auth_log(struct mg_str request, int status,
                           const char *response)
{
  char request_text[2048];
  char line[8192];
  char timestamp[32] = "unknown";
  const time_t now = time(NULL);
  struct tm *utc = gmtime(&now);
  size_t request_length = request.len < sizeof(request_text) - 1
                              ? request.len
                              : sizeof(request_text) - 1;
  long log_size;
  FILE *file;

  if (utc != NULL)
  {
    strftime(timestamp, sizeof(timestamp), "%Y-%m-%dT%H:%M:%SZ", utc);
  }
  if (request_length > 0)
  {
    memcpy(request_text, request.buf, request_length);
  }
  request_text[request_length] = '\0';

  mg_snprintf(line, sizeof(line),
              "{\"time\":%m,\"status\":%d,\"requestJson\":%m,"
              "\"responseJson\":%m}\n",
              MG_ESC(timestamp), status, MG_ESC(request_text),
              MG_ESC(response));

  file = fopen(AUTH_LOG_FILE, "a+");
  if (file == NULL)
  {
    MG_ERROR(("Cannot open %s", AUTH_LOG_FILE));
    return;
  }

  if (fseek(file, 0, SEEK_END) != 0)
  {
    MG_ERROR(("Cannot get size of %s", AUTH_LOG_FILE));
    fclose(file);
    return;
  }
  log_size = ftell(file);
  if (log_size < 0)
  {
    MG_ERROR(("Cannot get size of %s", AUTH_LOG_FILE));
    fclose(file);
    return;
  }
  if ((unsigned long long)log_size + strlen(line) > AUTH_LOG_MAX_SIZE)
  {
    fclose(file);
    remove(AUTH_LOG_BACKUP);
    if (rename(AUTH_LOG_FILE, AUTH_LOG_BACKUP) != 0)
    {
      MG_ERROR(("Cannot rotate %s", AUTH_LOG_FILE));
      file = fopen(AUTH_LOG_FILE, "w");
    }
    else
    {
      file = fopen(AUTH_LOG_FILE, "a");
    }
    if (file == NULL)
    {
      MG_ERROR(("Cannot reopen %s", AUTH_LOG_FILE));
      return;
    }
  }
  fputs(line, file);
  fclose(file);
}

static bool read_request(struct mg_str body, struct auth_request *request)
{
  if (mg_json_get_long(body, "$.version", 0) != 1)
  {
    return false;
  }
  if (mg_json_unescape(body, "$.deviceId", request->device_id, sizeof(request->device_id)) <= 0)
  {
    return false;
  }
  if (mg_json_unescape(body, "$.nonce", request->nonce, sizeof(request->nonce)) <= 0)
  {
    return false;
  }
  if (mg_json_unescape(body, "$.appVersion", request->app_version, sizeof(request->app_version)) <= 0)
  {
    return false;
  }
  if (mg_json_unescape(body, "$.platform", request->platform, sizeof(request->platform)) <= 0)
  {
    return false;
  }
  return true;
}

static bool device_allowed(const char *device_id)
{
  const char *allow_any = getenv("RIDEX_AUTH_ALLOW_ANY");
  char line[128];
  FILE *file;

  if (allow_any != NULL && strcmp(allow_any, "1") == 0)
  {
    return true;
  }
  file = fopen("devices.txt", "r");
  if (file == NULL)
  {
    return false;
  }
  while (fgets(line, sizeof(line), file) != NULL)
  {
    char *start = line;
    char *end;

    while (*start == ' ' || *start == '\t')
    {
      start++;
    }
    if (*start == '\0' || *start == '#' || *start == '\r' || *start == '\n')
    {
      continue;
    }
    end = start + strcspn(start, "\r\n \t");
    *end = '\0';
    if (strcmp(start, device_id) == 0)
    {
      fclose(file);
      return true;
    }
  }
  fclose(file);
  return false;
}

static bool signing_key(uint8_t key[32])
{
  char encoded[45] = {0};
  char line[128];
  uint8_t decoded[33];
  FILE *file = fopen(AUTH_KEY_FILE, "r");

  if (file == NULL)
  {
    return false;
  }

  while (fgets(line, sizeof(line), file) != NULL)
  {
    size_t prefix_length = strlen(AUTH_PRIVATE_KEY_PREFIX);
    if (strncmp(line, AUTH_PRIVATE_KEY_PREFIX, prefix_length) == 0)
    {
      char *value = line + prefix_length;
      size_t value_length;

      value[strcspn(value, "\r\n")] = '\0';
      value_length = strlen(value);
      if (value_length >= sizeof(encoded))
      {
        fclose(file);
        return false;
      }
      memcpy(encoded, value, value_length + 1);
      break;
    }
  }
  fclose(file);

  if (encoded[0] == '\0')
  {
    return false;
  }
  if (mg_base64url_decode(encoded, strlen(encoded), (char *)decoded, sizeof(decoded)) != 32)
  {
    return false;
  }
  memcpy(key, decoded, 32);
  mg_bzero(decoded, sizeof(decoded));
  return true;
}

static bool make_ticket(const struct auth_request *request, char *ticket,
                        size_t ticket_size, time_t *issued_at,
                        time_t *expires_at)
{
  uint8_t key[32], hash[32], signature[64];
  char payload[320], encoded[430], encoded_signature[89];
  time_t now = time(NULL);
  time_t expiration = now + AUTH_TICKET_LIFETIME_SECONDS;
  size_t payload_length, encoded_length, signature_length;

  if (!signing_key(key))
  {
    return false;
  }
  payload_length = (size_t)snprintf(payload, sizeof(payload),
                                    "{\"version\":1,\"issuer\":\"ridex-auth\",\"audience\":\"ridex-app\","
                                    "\"deviceId\":\"%s\",\"nonce\":\"%s\",\"issuedAt\":%lld,"
                                    "\"expiresAt\":%lld,\"features\":[]}",
                                    request->device_id, request->nonce, (long long)now,
                                    (long long)expiration);

  if (payload_length >= sizeof(payload))
  {
    return false;
  }
  encoded_length = mg_base64url_encode((uint8_t *)payload, payload_length, encoded, sizeof(encoded));
  if (encoded_length == 0)
  {
    return false;
  }
  mg_sha256(hash, (uint8_t *)encoded, encoded_length);

  if (mg_uecc_sign(key, hash, sizeof(hash), signature, mg_uecc_secp256r1()) != 1)
  {
    return false;
  }
  mg_bzero(key, sizeof(key));
  signature_length = mg_base64url_encode(signature, sizeof(signature), encoded_signature, sizeof(encoded_signature));
  if (signature_length == 0 || encoded_length + signature_length + 2 > ticket_size)
  {
    return false;
  }
  snprintf(ticket, ticket_size, "%s.%s", encoded, encoded_signature);
  if (issued_at != NULL)
  {
    *issued_at = now;
  }
  if (expires_at != NULL)
  {
    *expires_at = expiration;
  }
  return true;
}

static void event_handler(struct mg_connection *c, int event, void *data)
{
  struct mg_http_message *http = (struct mg_http_message *)data;
  struct auth_request request = {0};
  char ticket[520];
  char response[600];
  time_t issued_at = 0;
  time_t expires_at = 0;

  if (event != MG_EV_HTTP_MSG)
  {
    return;
  }
  if (!mg_match(http->uri, mg_str("/v1/app/authorize"), NULL))
  {
    write_auth_log(http->body, 404, "{\"message\":\"not found\"}");
    mg_http_reply(c, 404, JSON_HEADERS, "{\"message\":%m}", MG_ESC("not found"));
  }
  else if (mg_strcmp(http->method, mg_str("POST")) != 0)
  {
    write_auth_log(http->body, 405, "{\"message\":\"method not allowed\"}");
    mg_http_reply(c, 405, JSON_HEADERS, "{\"message\":%m}", MG_ESC("method not allowed"));
  }
  else if (!read_request(http->body, &request))
  {
    write_auth_log(http->body, 400, "{\"message\":\"invalid request\"}");
    mg_http_reply(c, 400, JSON_HEADERS, "{\"message\":%m}", MG_ESC("invalid request"));
  }
  else if (!device_allowed(request.device_id))
  {
    write_auth_log(http->body, 403, "{\"message\":\"device denied\"}");
    mg_http_reply(c, 403, JSON_HEADERS, "{\"message\":%m}", MG_ESC("device denied"));
  }
  else if (!make_ticket(&request, ticket, sizeof(ticket), &issued_at, &expires_at))
  {
    write_auth_log(http->body, 500, "{\"message\":\"ticket signing failed\"}");
    mg_http_reply(c, 500, JSON_HEADERS, "{\"message\":%m}", MG_ESC("ticket signing failed"));
  }
  else
  {
    char log_response[256];

    snprintf(response, sizeof(response),
             "{\"code\":\"authorized\",\"ticket\":\"%s\"}", ticket);
    snprintf(log_response, sizeof(log_response),
             "{\"code\":\"authorized\",\"ticket\":\"<redacted>\","
             "\"issuedAt\":%lld,\"expiresAt\":%lld,"
             "\"durationSeconds\":%lld}",
             (long long)issued_at, (long long)expires_at,
             (long long)(expires_at - issued_at));
    write_auth_log(http->body, 200, log_response);
    mg_http_reply(c, 200, JSON_HEADERS, "%s", response);
  }
}

static int generate_key(void)
{
  uint8_t private_key[32], public_key[65];
  char private_text[45], public_text[90];
  public_key[0] = 4;
  if (mg_uecc_make_key(public_key + 1, private_key, mg_uecc_secp256r1()) != 1)
  {
    return 1;
  }
  if (mg_base64url_encode(private_key, sizeof(private_key), private_text, sizeof(private_text)) == 0)
  {
    return 1;
  }
  if (mg_base64url_encode(public_key, sizeof(public_key), public_text, sizeof(public_text)) == 0)
  {
    return 1;
  }
  printf("RIDEX_AUTH_PRIVATE_KEY=%s\nRIDEX_AUTH_PUBLIC_KEY=%s\n", private_text, public_text);
  mg_bzero(private_key, sizeof(private_key));
  return 0;
}

int main(int argc, char **argv)
{
  struct mg_mgr manager;
  if (argc == 2 && strcmp(argv[1], "--generate-key") == 0)
  {
    return generate_key();
  }

  mg_log_set(MG_LL_INFO);
  mg_mgr_init(&manager);

  if (mg_http_listen(&manager, "http://127.0.0.1:8080", event_handler, NULL) == NULL)
  {
    fprintf(stderr, "cannot listen on 127.0.0.1:8080\n");
    return 1;
  }
  MG_INFO(("RideX auth server: http://127.0.0.1:8080"));

  for (;;)
  {
    mg_mgr_poll(&manager, 1000);
  }
}
