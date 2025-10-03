#!/bin/bash
# Test script for NWC connection requests
# This simulates an app requesting a connection and polling for approval

PORT=3737
URL="http://localhost:${PORT}"
POLL_INTERVAL=2  # seconds between polls
MAX_ATTEMPTS=30  # max number of poll attempts (60 seconds total)

echo "Testing NWC Connection Request & Polling"
echo "========================================"
echo ""

# Step 1: Request a connection
echo "Step 1: Requesting connection..."
echo "GET ${URL}"
echo ""

response=$(curl -s -w "\nHTTP_STATUS:%{http_code}" -X GET "${URL}")

# Extract the response body and status code
http_body=$(echo "$response" | sed -e 's/HTTP_STATUS\:.*//g')
http_status=$(echo "$response" | tr -d '\n' | sed -e 's/.*HTTP_STATUS://')

echo "Response Status: ${http_status}"
echo "Response Body:"
echo "${http_body}" | jq '.' 2>/dev/null || echo "${http_body}"
echo ""

if [ "$http_status" -ne 200 ]; then
    echo "❌ Request failed with status ${http_status}"
    exit 1
fi

# Extract request_id from response
request_id=$(echo "${http_body}" | jq -r '.request_id' 2>/dev/null)

if [ -z "$request_id" ] || [ "$request_id" = "null" ]; then
    echo "❌ Failed to extract request_id from response"
    exit 1
fi

echo "✅ Connection request created!"
echo "Request ID: ${request_id}"
echo ""
echo "👀 Check your wallet UI to approve the connection request."
echo ""

# Step 2: Poll for connection approval
echo "Step 2: Polling for approval..."
echo "Polling URL: ${URL}/poll/${request_id}"
echo ""

attempt=0
while [ $attempt -lt $MAX_ATTEMPTS ]; do
    attempt=$((attempt + 1))
    echo -n "Poll attempt ${attempt}/${MAX_ATTEMPTS}... "
    
    # Poll the connection status
    poll_response=$(curl -s -w "\nHTTP_STATUS:%{http_code}" -X GET "${URL}/poll/${request_id}")
    poll_body=$(echo "$poll_response" | sed -e 's/HTTP_STATUS\:.*//g')
    poll_status=$(echo "$poll_response" | tr -d '\n' | sed -e 's/.*HTTP_STATUS://')
    
    if [ "$poll_status" -ne 200 ]; then
        echo ""
        echo "❌ Poll failed with status ${poll_status}"
        echo "Response: ${poll_body}"
        exit 1
    fi
    
    # Extract status from response
    connection_status=$(echo "${poll_body}" | jq -r '.status' 2>/dev/null)
    
    case "$connection_status" in
        "approved")
            echo ""
            echo ""
            echo "🎉 Connection approved!"
            echo ""
            nwc_uri=$(echo "${poll_body}" | jq -r '.nwc_uri' 2>/dev/null)
            if [ -n "$nwc_uri" ] && [ "$nwc_uri" != "null" ]; then
                echo "NWC Connection String:"
                echo "======================"
                echo "${nwc_uri}"
                echo ""
                echo "✅ You can now use this connection string in your NWC client!"
            else
                echo "⚠️  Connection approved but NWC URI not available"
            fi
            exit 0
            ;;
        "rejected")
            echo ""
            echo ""
            echo "❌ Connection rejected by user"
            exit 1
            ;;
        "pending")
            echo "pending (waiting for user approval)"
            sleep $POLL_INTERVAL
            ;;
        "not_found")
            echo ""
            echo "❌ Connection request not found or expired"
            exit 1
            ;;
        *)
            echo "unknown status: ${connection_status}"
            sleep $POLL_INTERVAL
            ;;
    esac
done

echo ""
echo "⏱️  Timeout: Connection was not approved within ${MAX_ATTEMPTS} attempts"
echo "The connection request may have expired."
exit 1
