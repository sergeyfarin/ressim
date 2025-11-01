# Tooltip Fix - onCanvasMouseMove Handler

## Issue
The tooltip with cell properties (pressure, water saturation, oil saturation) stopped working when hovering over grid cells.

## Root Cause
The `onCanvasMouseMove` handler was directly checking `activeGrid` in the initial validation, but `activeGrid` is a reactive variable that might not be synchronized with the handler execution. This caused the handler to exit early with `tooltipVisible = false`.

## Solution
Modified the `onCanvasMouseMove` function in `src/lib/3dview.svelte` to:

1. **Remove strict `activeGrid` check**: Instead of checking `!activeGrid` in the initial guard, we now only validate basic Three.js objects
   
2. **Call `getActiveGrid()` function**: Inside the handler, we now explicitly call `getActiveGrid()` which properly handles both:
   - History mode: returns `history[currentIndex].grid`
   - Live mode: returns current `gridState`

3. **Add proper null checks**: The new `currentGrid` is checked to ensure it exists and has length > 0

4. **Improved raycaster call**: Changed from `raycaster.intersectObject(instancedMesh)` to `raycaster.intersectObject(instancedMesh, false)` for clarity (recursive: false since we only need direct children)

## Code Changes

### Before
```typescript
function onCanvasMouseMove(event: MouseEvent): void {
    if (!renderer || !scene || !camera || !instancedMesh || !canvasContainer || !activeGrid) {
        tooltipVisible = false;
        return;
    }
    // ... rest of handler uses activeGrid directly
}
```

### After
```typescript
function onCanvasMouseMove(event: MouseEvent): void {
    if (!renderer || !scene || !camera || !instancedMesh || !canvasContainer) {
        tooltipVisible = false;
        return;
    }

    // Get the active grid - use current gridState or history entry
    const currentGrid = getActiveGrid();
    if (!currentGrid || currentGrid.length === 0) {
        tooltipVisible = false;
        return;
    }
    // ... rest of handler uses currentGrid
}
```

## Why This Works

- **Reactive Variables**: `activeGrid` is computed reactively but may have stale values during mouse events
- **Explicit Function Call**: `getActiveGrid()` always returns the correct grid at runtime
- **History Support**: Works correctly both when replaying history and in live simulation mode
- **Safe Access**: Multiple null checks ensure no errors even if grid data is incomplete

## Testing

The tooltip should now appear when hovering over grid cells, showing:
- Pressure: [value with 2 decimal places]
- Water Sat: [value with 3 decimal places]
- Oil Sat: [value with 3 decimal places]

Works in both:
✅ Live simulation mode (showing `gridState`)
✅ History replay mode (showing `history[currentIndex].grid`)
