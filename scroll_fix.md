# Room Message Loading and Scroll Fix Report

This report identifies the root causes of lagging, scroll issues, and tab-switching friction in the `fluxchat` application, along with recommended architectural fixes.

## Core Issues Identified

### 1. Excessive Initial Loading
The `select_room` function in `crates/app/src/components/sidebar/room_list.rs` currently runs a loop that fetches up to **5 batches (100 messages)** just to ensure it has at least 5 "real" messages. This contradicts the requirement to only load the latest messages and causes unnecessary network and rendering overhead.

### 2. Rendering All Open Tabs Simultaneously
In `crates/app/src/components/chat/message_area.rs`, the code renders the message list for **every open tab** at all times, merely hiding inactive ones with CSS (`display: none`).
*   **Impact:** As more tabs are opened, the DOM grows massive. With 8 tabs each containing 100+ messages and images, the browser struggles to maintain performance, causing lag during tab switching and scrolling.

### 3. Fragmented and Hacky Scroll Logic
The "scroll to bottom" logic is implemented in three different places (`app.rs`, `message_area.rs`, and `room_list.rs`) using varying combinations of `setTimeout` (100ms, 300ms, 600ms) and `requestAnimationFrame`.
*   **Impact:** These "brute-force" scroll attempts fight with each other and the Dioxus rendering cycle, leading to jumpy scrolling or the scrollbar failing to stay at the bottom when new messages arrive.

### 4. Inefficient Lazy Loading (Scroll Restoration)
The `onscroll` handler in `RoomMessages` attempts to restore the scroll position after loading older messages using `el.scrollTop = el.scrollHeight - old_height`. However, it is wrapped in multiple `requestAnimationFrame` calls which are often out of sync with when Dioxus actually updates the DOM.
*   **Impact:** This results in "scroll issues" where the view jumps or loses its place when scrolling up to load history.

### 5. Expensive Rendering Computations
The `transform_mentions` function (which handles mention pills and avatars) is called on **every single render** for **every message**.
*   **Impact:** This heavy string manipulation task happens hundreds of times per second during a scroll, contributing significantly to UI lag.

---

## Recommended Fixes

### 1. Conditional Tab Rendering
Change `MessageArea` to **only render the active tab** in the DOM. Instead of using `display: none`, the `RoomMessages` component should only be instantiated for the currently selected room. This will immediately resolve the performance scaling issue.

### 2. Simplify Initial Load
Modify the logic in `select_room` to fetch only **one batch** (20 messages) initially. Only trigger further fetches if the user explicitly scrolls up.

### 3. Unified Scroll Management
Centralize the scroll-to-bottom logic. Instead of scattered `setTimeout` calls, use a single `use_effect` that triggers specifically when the `messages` signal changes for the active room or when the room is switched.

### 4. Memoize Mentions and Message Bodies
Pre-calculate the transformed HTML for mentions when a message is first received and store it, or wrap the message body in a `memo` component to prevent redundant parsing during every render cycle.

### 5. Refine Scroll Restoration
Use a more reliable method for scroll restoration during pagination, such as a `ResizeObserver` or a more targeted JS injection that executes exactly once after the DOM update.

### 6. Throttle Scroll Events
The `onscroll` listener should be throttled or debounced to avoid triggering too many expensive `document::eval` calls while the user is actively scrolling.
