# Prompt hover cancellation from a different DOM row

## Field evidence

Captured on 2026-10-05 in the installed macOS app, with temporary opt-in
`DEBUG-hvr` instrumentation. The decisive trace is saved outside the repository
at `/private/tmp/sleepy-cat-hover-20261004-1838/confirmed-dom-leave-excerpt.jsonl`.

- At 18:20:07.453, the highlight and full-content preview were committed for row A.
- At 18:20:07.657, a DOM `pointerleave` handler for a different row B ran. Its
  coordinates were `(64, 345)`, while the latest native pointer was
  `(85.73828125, 254.16015625)` on row A.
- The handler unconditionally called the shared hover cleanup, clearing row A's
  highlight, preview, anchor, and timer.
- At 18:20:07.759, an independent native pointer snapshot still reported
  `(85.73828125, 254.16015625)`, inside row A's rectangle
  `(10, 229)-(270, 287)`.
- The next native movement report restored row A's highlight and armed a new
  preview timer. Clearing internal state alone does not rerun pointer reconciliation.

The cancellation was caused by a row-local event acting on state owned by
another row. The non-activating WebView's DOM hover target was inconsistent with
the native tracking target. This capture does not establish which internal
WebKit/AppKit operation caused that disagreement, nor does it prove every
historical flicker had the same trigger.

## Change

Before accepting a DOM leave or cancellation, check ownership against the current
preview anchor's element. When scrolling has cleared that anchor but retained
the highlight, check against the highlighted prompt ID instead. Ignore events
from unrelated rows without clearing the current pointer or preview.

Retain native tracking, the 1500 ms preview delay, immediate cleanup for a leave
or cancellation of the owning row, native window exits, scrolling, reset, and
selection. No added debounce or IPC round trip is needed for this captured path.

## Validation

Four regression cases failed before the change and pass after it:

- A native-established single-row preview survives another row's DOM leave.
- The same behavior holds for a group-row preview.
- Another row's leave cannot cancel the pending preview countdown.
- DOM cancellation is likewise limited to the owning row.

The cases use no DOM pointermove to establish hover, matching the observed
non-activating WebView behavior. They also verify that leaving or cancelling the
owning row still clears immediately. An additional case verifies highlight
cleanup after scrolling has removed the preview anchor. Both PromptQuickList
test files pass: 39 tests. `npm run build` and the native release build pass.

The installed diagnostic fix subsequently recorded an unrelated row's DOM leave
being ignored while the current row retained its highlight and pending preview
timer. This corroborates the ownership guard in the running app; it does not
establish that every historical flicker had the same cause. Temporary diagnostic
code was removed before packaging v1.3.5.
