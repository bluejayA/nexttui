# Stage 3: Security/Edge-case Review (Gemini 2.5 Pro)

## Status: CONDITIONAL

## Critical Issues
1. **Thundering Herd in get_token**: Multiple concurrent callers hitting expired token all trigger refresh_token simultaneously — storm of auth requests to Keystone.

## Warnings
1. **BaseHttpClient endpoint cache never invalidated**: invalidate_endpoint exists but never called. Token refresh fetches new catalog but stale endpoints persist.
2. **Refresh loop can spawn multiple times**: No guard prevents multiple authenticate() calls from spawning redundant background tasks.

## Edge Cases (acceptable)
1. broadcast channel sends full Token to all subscribers — acceptable for internal tool
2. Repeated refresh failures leave user seeing errors only on action — no proactive "disconnected" UI state

## Recommendations
1. Wrap get_token refresh in Mutex or single-flight pattern to prevent thundering herd
2. BaseHttpClient should subscribe to token refresh broadcast and call invalidate_endpoint
3. Use OnceCell or guard to ensure refresh loop spawns only once

## Overall Security Assessment: LOW to MEDIUM
Well-structured credential handling, masked Debug, proper timeouts, auth delegation. Risks are race conditions and state management, not fundamental security flaws.
