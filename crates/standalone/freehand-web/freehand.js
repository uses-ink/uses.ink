// The loader of the freehand pen in the browser: the only JS the pen needs. Each mark returns
// its SVG path data, one string per path. The protocol is in `src/lib.rs`.
const text = new TextEncoder()
const read = new TextDecoder()

export async function load(url) {
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url))
  const pen = instance.exports
  // writes points as f64 pairs, then the name; returns the name's length in bytes
  const put = (points, name) => {
    const bytes = text.encode(name)
    const at = pen.input(points.length * 16 + bytes.length)
    const floats = new Float64Array(pen.memory.buffer, at, points.length * 2)
    points.forEach(([x, y], i) => floats.set([x, y], i * 2))
    new Uint8Array(pen.memory.buffer, at + points.length * 16, bytes.length).set(bytes)
    return bytes.length
  }
  const paths = (length) => {
    if (length === 0) return []
    return read.decode(new Uint8Array(pen.memory.buffer, pen.output(), length)).split('\n')
  }
  return {
    line: ([fx, fy], [tx, ty], weight, name) => paths(pen.line(fx, fy, tx, ty, weight, put([], name))),
    underline: (width, y, weight, name) => paths(pen.underline(width, y, weight, put([], name))),
    frame: (width, height, slack, weight, name) => paths(pen.frame(width, height, slack, weight, put([], name))),
    fill: (width, height, [sx, sy], name) => paths(pen.fill(width, height, sx, sy, put([], name))),
    patch: (corners, name) => paths(pen.patch(corners.length, put(corners, name))),
    drawn: (points, weight, { wobble = 1, tremor = 0.9 }, name) =>
      paths(pen.drawn(points.length, weight, wobble, tremor, put(points, name))),
  }
}
