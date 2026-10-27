#!/usr/bin/env bash
# End-to-end smoke test for the REST API (and optionally the React bundle).
#
#   BASE_URL=http://127.0.0.1:8080 ./scripts/smoke.sh
#   BASE_URL=https://spell.lab CURL_OPTS=-sk CHECK_WEB=1 ./scripts/smoke.sh
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"
BASE_URL="${BASE_URL%/}"
if [[ "$BASE_URL" == */api/v1 ]]; then
  API="$BASE_URL"
else
  API="$BASE_URL/api/v1"
fi
CURL_OPTS="${CURL_OPTS:--sS}"
CHECK_WEB="${CHECK_WEB:-0}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

pass=0
fail=0

ok() {
  pass=$((pass + 1))
  printf '  \033[32mok\033[0m   %s\n' "$1"
}

bad() {
  fail=$((fail + 1))
  printf '  \033[31mFAIL\033[0m %s\n' "$1"
}

# request METHOD PATH [JSON] [API_KEY] -> writes body to $TMP/body and echoes code
request() {
  local method="$1" path="$2" data="${3:-}" key="${4:-}"
  local args=($CURL_OPTS -o "$TMP/body" -w '%{http_code}' -X "$method" "$API$path")
  if [[ -n "$data" ]]; then
    args+=(-H 'content-type: application/json' -d "$data")
  fi
  if [[ -n "$key" ]]; then
    args+=(-H "x-api-key: $key")
  fi
  curl "${args[@]}"
}

json_get() {
  python3 -c "import json,sys; d=json.load(open('$TMP/body')); print($1)" 2>/dev/null
}

assert_eq() {
  if [[ "$2" == "$3" ]]; then ok "$1"; else bad "$1 (expected '$3', got '$2')"; fi
}

assert_true() {
  if [[ "$2" == "True" || "$2" == "true" ]]; then ok "$1"; else bad "$1 (got '$2')"; fi
}

assert_python() {
  if python3 -c "$2" < "$TMP/body" 2>/dev/null; then ok "$1"; else bad "$1"; fi
}

printf '\nSpell.lab smoke test\n  API: %s\n\n' "$API"

code="$(request GET /health)"
assert_eq "health returns 200" "$code" "200"
assert_eq "health status is ok" "$(json_get "d['status']")" "ok"
assert_true "lexicon contains >300k words" "$(json_get "d['globalWords'] > 300000")"

code="$(request POST /spell/suggest '{"word":"teh","maxSuggestions":5,"maxDistance":2}')"
assert_eq "suggest(teh) returns 200" "$code" "200"
assert_eq "suggest(teh) top result" "$(json_get "d['suggestions'][0]['word']")" "the"
assert_eq "suggest(teh) reason" "$(json_get "d['suggestions'][0]['reason']")" "transposition"

code="$(request POST /spell/suggest '{"word":"odre","maxSuggestions":5,"maxDistance":2}')"
assert_eq "suggest(odre) returns 200" "$code" "200"
assert_python "suggest(odre) contains order" \
  "import json,sys; d=json.load(sys.stdin); assert any(s['word']=='order' for s in d['suggestions'])"

code="$(request POST /users '{"name":"smoke-test"}')"
assert_eq "create user returns 200" "$code" "200"
KEY="$(json_get "d['apiKey']")"
[[ "$KEY" == sl_* ]] && ok "API key has sl_ prefix" || bad "API key has sl_ prefix"

code="$(request POST /dictionary/words '{"words":["kubernets","grafana"]}' "$KEY")"
assert_eq "add words returns 200" "$code" "200"
assert_eq "two words added" "$(json_get "d['added']")" "2"

code="$(request POST /spell/check '{"text":"kubernets grafana","maxSuggestions":3}' "$KEY")"
assert_eq "check with user dictionary returns 200" "$code" "200"
assert_true "user word is correct" "$(json_get "d['tokens'][0]['correct']")"
assert_true "user word is flagged as user source" "$(json_get "d['tokens'][0]['inUserDictionary']")"

code="$(request GET /dictionary "" "$KEY")"
assert_eq "list dictionary returns 200" "$code" "200"
assert_eq "dictionary total is 2" "$(json_get "d['total']")" "2"

code="$(request POST /spell/correct '{"text":"Teh quick brown fox","minConfidence":85}')"
assert_eq "correct returns 200" "$code" "200"
assert_eq "correct fixes Teh" "$(json_get "d['corrected']")" "The quick brown fox"

code="$(request POST /users/me/rotate-key "" "$KEY")"
assert_eq "rotate key returns 200" "$code" "200"
NEW_KEY="$(json_get "d['apiKey']")"
assert_true "rotated key differs" "$(python3 -c "print('$NEW_KEY' != '$KEY')")"

code="$(request GET /users/me "" "$KEY")"
assert_eq "old key is rejected" "$code" "401"
code="$(request GET /users/me "" "$NEW_KEY")"
assert_eq "new key works" "$code" "200"

code="$(request GET /stats)"
assert_eq "stats returns 200" "$code" "200"
assert_true "cache capacity is exposed" "$(json_get "d['suggestionCache']['capacity'] > 0")"

code="$(request DELETE /users/me "" "$NEW_KEY")"
assert_eq "delete identity returns 200" "$code" "200"
code="$(request GET /users/me "" "$NEW_KEY")"
assert_eq "deleted key is rejected" "$code" "401"

if [[ "$CHECK_WEB" == "1" ]]; then
  code="$(curl $CURL_OPTS -o "$TMP/body" -w '%{http_code}' "$BASE_URL/")"
  assert_eq "React index returns 200" "$code" "200"
  grep -q '<div id="root">' "$TMP/body" && ok "React root element present" || bad "React root element present"
fi

printf '\n%s passed, %s failed\n\n' "$pass" "$fail"
[[ "$fail" -eq 0 ]]
