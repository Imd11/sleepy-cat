# Calico Rich Idle Motion Director QA

Date: 2026-07-05

## Automated Browser Check

Command path: Chrome DevTools Protocol against `http://127.0.0.1:1420/overlay.html`.

Results:

- Initial overlay state: `idle-follow`
- Initial sprite loaded: `/calico/calico-idle-follow.svg`, natural size `1536x1152`
- Deterministic idle check with `Math.random = () => 0`: state became `idle` after the first idle delay
- Idle sprite loaded: `/calico/calico-idle.apng`, natural size `266x200`
- Plan-required mini and sleep-family states loaded with valid layout bounds:
  - `sleeping`
  - `waking`
  - `mini-enter`
  - `mini-idle`
  - `mini-peek`
  - `mini-happy`
  - `mini-crabwalk`
  - `mini-sleep`
  - `mini-alert`
- Each plan-required visual state loaded with positive natural dimensions and at least `132x132` visible viewport coverage in the automated browser check.
- Non-favicon network errors: none
- Relevant console errors: none
- Ignored browser request: `/favicon.ico` returned 404
- Screenshot artifact: `/tmp/prompt-picker-calico-idle-review-fix-overlay.png`
