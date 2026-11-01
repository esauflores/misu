
function length(a, b) {
    return Math.hypot(b.x - a.x, b.y - a.y);
}

function unit(a, b) {
    const d = length(a, b);
    return d > 0 ? { x: (b.x - a.x) / d, y: (b.y - a.y) / d } : { x: 0, y: 0 };
}

function number(value) {
    const rounded = Math.round(value * 1000) / 1000;
    return String(Object.is(rounded, -0) ? 0 : rounded);
}

function pointCommand(command, point) {
    return command + " " + number(point.x) + " " + number(point.y);
}

function roundedPath(points, radius) {
    const count = points.length;
    if (count < 3) return "";

    const entering = [];
    const leaving = [];
    for (let index = 0; index < count; ++index) {
        const previous = points[(index - 1 + count) % count];
        const current = points[index];
        const next = points[(index + 1) % count];
        const before = unit(current, previous);
        const after = unit(current, next);
        const requested = Array.isArray(radius) ? radius[index] : radius;
        const amount = Math.max(0, Math.min(requested, length(previous, current) / 2,
            length(current, next) / 2));
        entering.push({ x: current.x + before.x * amount, y: current.y + before.y * amount });
        leaving.push({ x: current.x + after.x * amount, y: current.y + after.y * amount });
    }

    let path = pointCommand("M", leaving[0]);
    for (let step = 1; step <= count; ++step) {
        const index = step % count;
        path += " " + pointCommand("L", entering[index]);
        path += " Q " + number(points[index].x) + " " + number(points[index].y)
            + " " + number(leaving[index].x) + " " + number(leaving[index].y);
    }
    return path + " Z";
}

function framePath(width, height, points, radius) {
    const outer = "M 0 0 H " + number(width) + " V " + number(height) + " H 0 Z";
    const hole = roundedPath(points, radius);
    return hole === "" ? outer : outer + " " + hole;
}

if (typeof module !== "undefined" && module.exports)
    module.exports = { roundedPath, framePath };
