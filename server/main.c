#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mongoose.h"

#define JSON_HEADERS "Content-Type: application/json\r\n"

static const uint8_t DEV_KEY[32] = {
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1};

struct auth_request
{
  char device_id[44];
  char nonce[44];
  char app_version[32];
  char platform[16];
};

static void reject(struct mg_connection *c, int status, const char *message)
{
  mg_http_reply(c, status, JSON_HEADERS, "{\"message\":%m}", MG_ESC(message));
}

static bool read_request(struct mg_str body, struct auth_request *request)
{
  return mg_json_get_long(body, "$.version", 0) == 1 &&
         mg_json_unescape(body, "$.deviceId", request->device_id,
                          sizeof(request->device_id)) > 0 &&
         mg_json_unescape(body, "$.nonce", request->nonce,
                          sizeof(request->nonce)) > 0 &&
         mg_json_unescape(body, "$.appVersion", request->app_version,
                          sizeof(request->app_version)) > 0 &&
         mg_json_unescape(body, "$.platform", request->platform,
                          sizeof(request->platform)) > 0;
}

static bool device_allowed(const char *device_id)
{
  const char *allow_any = getenv("RIDEX_AUTH_ALLOW_ANY");
  const char *path = getenv("RIDEX_AUTH_DEVICES");
  char line[128];
  FILE *file;

  if (allow_any != NULL && strcmp(allow_any, "1") == 0)
  {
    return true;
  }
  file = fopen(path == NULL ? "devices.txt" : path, "r");
  if (file == NULL)
  {
    return false;
  }
  while (fgets(line, sizeof(line), file) != NULL)
  {
    char *end = line + strcspn(line, "\r\n \t");
    *end = '\0';
    if (line[0] != '#' && strcmp(line, device_id) == 0)
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
  const char *encoded = getenv("RIDEX_AUTH_PRIVATE_KEY");
  uint8_t decoded[33];
  if (encoded == NULL)
  {
    memcpy(key, DEV_KEY, sizeof(DEV_KEY));
    return true;
  }
  if (mg_base64url_decode(encoded, strlen(encoded), (char *)decoded, sizeof(decoded)) != 32)
  {
    return false;
  }
  memcpy(key, decoded, 32);
  mg_bzero(decoded, sizeof(decoded));
  return true;
}

static bool make_ticket(const struct auth_request *request, char *ticket, size_t ticket_size)
{
  uint8_t key[32], hash[32], signature[64];
  char payload[320], encoded[430], encoded_signature[89];
  time_t now = time(NULL);
  size_t payload_length, encoded_length, signature_length;

  if (!signing_key(key))
  {
    return false;
  }
  payload_length = (size_t)snprintf(
      payload, sizeof(payload),
      "{\"version\":1,\"issuer\":\"ridex-auth\",\"audience\":\"ridex-app\","
      "\"deviceId\":\"%s\",\"nonce\":\"%s\",\"issuedAt\":%lld,"
      "\"expiresAt\":%lld,\"features\":[]}",
      request->device_id, request->nonce, (long long)now,
      (long long)now + 72 * 60 * 60);

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
  return true;
}

static void event_handler(struct mg_connection *c, int event, void *data)
{
  struct mg_http_message *http = (struct mg_http_message *)data;
  struct auth_request request = {0};
  char ticket[520];

  if (event != MG_EV_HTTP_MSG)
  {
    return;
  }
  if (!mg_match(http->uri, mg_str("/v1/app/authorize"), NULL))
  {
    reject(c, 404, "not found");
  }
  else if (mg_strcmp(http->method, mg_str("POST")) != 0)
  {
    reject(c, 405, "method not allowed");
  }
  else if (!read_request(http->body, &request))
  {
    reject(c, 400, "invalid request");
  }
  else if (!device_allowed(request.device_id))
  {
    MG_INFO(("Denied device: %s", request.device_id));
    reject(c, 403, "device denied");
  }
  else if (!make_ticket(&request, ticket, sizeof(ticket)))
  {
    reject(c, 500, "ticket signing failed");
  }
  else
  {
    mg_http_reply(c, 200, JSON_HEADERS,
                  "{\"code\":\"authorized\",\"ticket\":\"%s\"}", ticket);
  }
}

static int generate_key(void)
{
  uint8_t private_key[32], public_key[65];
  char private_text[45], public_text[90];
  public_key[0] = 4;
  if (mg_uecc_make_key(public_key + 1, private_key,
                       mg_uecc_secp256r1()) != 1 ||
      mg_base64url_encode(private_key, sizeof(private_key), private_text,
                          sizeof(private_text)) == 0 ||
      mg_base64url_encode(public_key, sizeof(public_key), public_text,
                          sizeof(public_text)) == 0)
    return 1;
  printf("RIDEX_AUTH_PRIVATE_KEY=%s\nRIDEX_AUTH_PUBLIC_KEY=%s\n", private_text,
         public_text);
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
