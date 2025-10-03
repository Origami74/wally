#!/bin/bash
# Test script for NWC connection requests
# This simulates an app sending a connection request to the wallet

PORT=3737
URL="http://localhost:${PORT}"

# Sample NWA connection string (from the unit tests)
NWA_URI="nostr+walletauth://b889ff5b1513b641e2a139f661a661364979c5beee91842f8f0ef42ab558e9d4?relay=ws%3A%2F%2Flocalhost%3A4869&secret=b8a30fafa48d4795b6c0eec169a383de&required_commands=pay_invoice%20make_invoice&optional_commands=list_transactions&budget=10000%2Fdaily"

echo "Testing NWC Connection Request"
echo "==============================="
echo ""
echo "Sending POST request to ${URL}"
echo ""

# Make the POST request
response=$(curl -s -w "\nHTTP_STATUS:%{http_code}" -X POST "${URL}" \
  -H "Content-Type: application/json" \
  -d "{\"nwa\": \"${NWA_URI}\"}")

# Extract the response body and status code
http_body=$(echo "$response" | sed -e 's/HTTP_STATUS\:.*//g')
http_status=$(echo "$response" | tr -d '\n' | sed -e 's/.*HTTP_STATUS://')

echo "Response Status: ${http_status}"
echo ""
echo "Response Body:"
echo "${http_body}" | jq '.' 2>/dev/null || echo "${http_body}"
echo ""

if [ "$http_status" -eq 200 ]; then
    echo "✅ Success! Check your wallet UI for the connection approval prompt."
else
    echo "❌ Request failed with status ${http_status}"
fi

