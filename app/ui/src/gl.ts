// The WebGL2 trace renderer. Each trace is uploaded once; pan and zoom only change two uniforms (clip = v * a + b
// on each axis), so redrawing a stale reply at the new view costs almost nothing while the sharper one is
// computed. NaN in y is a gap: a trace is drawn as separate line strips around it. One canvas covers the whole
// plot stack and the overview; each trace draws into its pane's rectangle.

const VERT = `#version 300 es
// A line is drawn as a quad per segment (WebGL's own lines are always one device pixel wide): instance k joins
// point k (a_x0, a_y0) to point k + 1 (a_x1, a_y1), and the quad is u_w device pixels wide. u_points draws single
// points as squares instead.
in float a_x0;
in float a_y0;
in float a_x1;
in float a_y1;
uniform vec2 u_x;
uniform vec2 u_y;
uniform vec2 u_vp;
uniform float u_w;
uniform bool u_points;
void main() {
  vec2 p0 = vec2(a_x0 * u_x.x + u_x.y, a_y0 * u_y.x + u_y.y);
  if (u_points) {
    gl_Position = vec4(p0, 0.0, 1.0);
    gl_PointSize = max(2.0, u_w * 1.6);
    return;
  }
  vec2 p1 = vec2(a_x1 * u_x.x + u_x.y, a_y1 * u_y.x + u_y.y);
  vec2 d = (p1 - p0) * u_vp * 0.5;
  float len = length(d);
  d = len > 1e-4 ? d / len : vec2(1.0, 0.0);
  vec2 n = vec2(-d.y, d.x);
  float side = (gl_VertexID & 1) == 0 ? -1.0 : 1.0;
  vec2 base = gl_VertexID < 2 ? p0 : p1;
  // half a width out sideways, and half a width along the line so joints between segments overlap
  float along = gl_VertexID < 2 ? -1.0 : 1.0;
  vec2 off = (n * side + d * along) * (u_w * 0.5);
  gl_Position = vec4(base + off * 2.0 / u_vp, 0.0, 1.0);
}`;

const FRAG = `#version 300 es
precision mediump float;
uniform vec4 u_color;
out vec4 color;
void main() {
  color = u_color;
}`;

/** A plot line's width in CSS pixels, as the prototype's uPlot lines: with 4x MSAA, quads this wide draw the same ink per
 * unit length as a 1.25 px canvas stroke (measured in WKWebView and Chrome at 1x and 2x). */
export const LINE_WIDTH = 1.25;

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** clip = v * a + b */
export interface Axis {
  a: number;
  b: number;
}

export type Rgba = [number, number, number, number];

/** One uploaded trace. */
export interface GlTrace {
  vao: WebGLVertexArrayObject;
  bufs: WebGLBuffer[];
  /** [start, count] of each run without NaN */
  runs: [number, number][];
}

export class Renderer {
  private gl: WebGL2RenderingContext;
  private prog: WebGLProgram;
  private uX: WebGLUniformLocation;
  private uY: WebGLUniformLocation;
  private uColor: WebGLUniformLocation;
  private uVp: WebGLUniformLocation;
  private uW: WebGLUniformLocation;
  private uPoints: WebGLUniformLocation;
  /** Lines as quads (a width of CSS pixels), or WebGL's own one-pixel lines: software GL (headless test browsers) draws quads slowly. */
  quads: boolean;
  private loc: Record<'a_x0' | 'a_y0' | 'a_x1' | 'a_y1', number>;

  constructor(private canvas: HTMLCanvasElement) {
    const gl = canvas.getContext('webgl2', { antialias: true, alpha: true, premultipliedAlpha: true });
    if (!gl) throw new Error('WebGL2 is not available');
    this.gl = gl;
    this.prog = link(gl, VERT, FRAG);
    this.uX = gl.getUniformLocation(this.prog, 'u_x')!;
    this.uY = gl.getUniformLocation(this.prog, 'u_y')!;
    const info = gl.getExtension('WEBGL_debug_renderer_info');
    const soft = /swiftshader|llvmpipe|software/i.test(String(info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : ''));
    const force = (window as unknown as { __tlmLines?: string }).__tlmLines;
    this.quads = force ? force === 'quads' : !soft;
    this.uColor = gl.getUniformLocation(this.prog, 'u_color')!;
    this.uVp = gl.getUniformLocation(this.prog, 'u_vp')!;
    this.uW = gl.getUniformLocation(this.prog, 'u_w')!;
    this.uPoints = gl.getUniformLocation(this.prog, 'u_points')!;
    this.loc = Object.fromEntries((['a_x0', 'a_y0', 'a_x1', 'a_y1'] as const).map((n) => [n, gl.getAttribLocation(this.prog, n)])) as Renderer['loc'];
  }

  /** Matches the drawing buffer to the canvas's size in device pixels, and clears it. */
  begin(): void {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(this.canvas.clientWidth * dpr));
    const h = Math.max(1, Math.round(this.canvas.clientHeight * dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
    const gl = this.gl;
    gl.viewport(0, 0, w, h);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
  }

  upload(x: Float32Array, y: Float32Array, reuse?: GlTrace): GlTrace {
    const gl = this.gl;
    const t = reuse ?? { vao: gl.createVertexArray()!, bufs: [gl.createBuffer()!, gl.createBuffer()!], runs: [] };
    gl.bindVertexArray(t.vao);
    for (const [k, data] of [[0, x], [1, y]] as const) {
      gl.bindBuffer(gl.ARRAY_BUFFER, t.bufs[k]);
      gl.bufferData(gl.ARRAY_BUFFER, data, gl.DYNAMIC_DRAW);
    }
    gl.bindVertexArray(null);
    t.runs = [];
    let start = -1;
    for (let i = 0; i <= y.length; i++) {
      const gap = i === y.length || Number.isNaN(y[i]);
      if (!gap && start < 0) start = i;
      if (gap && start >= 0) {
        t.runs.push([start, i - start]);
        start = -1;
      }
    }
    return t;
  }

  free(t: GlTrace): void {
    this.gl.deleteVertexArray(t.vao);
    t.bufs.forEach((b) => this.gl.deleteBuffer(b));
  }

  /** Draws `t` into `rect` (CSS pixels from the canvas's top left); clip space spans the rect. `width` is in CSS pixels. */
  draw(t: GlTrace, x: Axis, y: Axis, rgba: Rgba, rect: Rect, width = LINE_WIDTH): void {
    if (!t.runs.length || rect.w <= 0 || rect.h <= 0) return;
    const gl = this.gl;
    const dpr = this.canvas.width / Math.max(1, this.canvas.clientWidth);
    const [vx, vw, vh] = [Math.round(rect.x * dpr), Math.round(rect.w * dpr), Math.round(rect.h * dpr)];
    const vy = this.canvas.height - Math.round(rect.y * dpr) - vh;
    gl.viewport(vx, vy, vw, vh);
    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(vx, vy, vw, vh);
    gl.useProgram(this.prog);
    gl.uniform2f(this.uX, x.a, x.b);
    gl.uniform2f(this.uY, y.a, y.b);
    gl.uniform2f(this.uVp, vw, vh);
    gl.uniform1f(this.uW, width * dpr);
    gl.uniform4f(this.uColor, ...rgba);
    gl.bindVertexArray(t.vao);
    const { a_x0, a_y0, a_x1, a_y1 } = this.loc;
    // each attribute reads one buffer at a run's first point (the second pair one point on), once per instance
    const point = (loc: number, buf: WebGLBuffer, at: number, divisor: number) => {
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 1, gl.FLOAT, false, 0, at * 4);
      gl.vertexAttribDivisor(loc, divisor);
    };
    for (const [start, count] of t.runs) {
      if (count > 1 && !this.quads) {
        gl.uniform1i(this.uPoints, 1);
        point(a_x0, t.bufs[0], start, 0);
        point(a_y0, t.bufs[1], start, 0);
        point(a_x1, t.bufs[0], start, 0);
        point(a_y1, t.bufs[1], start, 0);
        gl.drawArrays(gl.LINE_STRIP, 0, count);
      } else if (count === 1) {
        gl.uniform1i(this.uPoints, 1);
        point(a_x0, t.bufs[0], start, 0);
        point(a_y0, t.bufs[1], start, 0);
        point(a_x1, t.bufs[0], start, 0);
        point(a_y1, t.bufs[1], start, 0);
        gl.drawArrays(gl.POINTS, 0, 1);
      } else {
        gl.uniform1i(this.uPoints, 0);
        point(a_x0, t.bufs[0], start, 1);
        point(a_y0, t.bufs[1], start, 1);
        point(a_x1, t.bufs[0], start + 1, 1);
        point(a_y1, t.bufs[1], start + 1, 1);
        gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, count - 1);
      }
    }
    gl.bindVertexArray(null);
    gl.disable(gl.SCISSOR_TEST);
  }
}

function link(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const shader = (type: number, src: string) => {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? 'shader');
    return s;
  };
  const p = gl.createProgram()!;
  gl.attachShader(p, shader(gl.VERTEX_SHADER, vs));
  gl.attachShader(p, shader(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? 'link');
  return p;
}
