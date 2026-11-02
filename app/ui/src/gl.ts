// The WebGL2 trace renderer. A trace is uploaded once; pan and zoom only change two uniforms (clip = v * a + b
// on each axis), so redrawing a stale reply at the new view costs almost nothing while the sharper one is
// computed. NaN in y is a gap: the trace is drawn as separate line strips around it.

const VERT = `#version 300 es
in float a_x;
in float a_y;
uniform vec2 u_x;
uniform vec2 u_y;
void main() {
  gl_Position = vec4(a_x * u_x.x + u_x.y, a_y * u_y.x + u_y.y, 0.0, 1.0);
}`;

const FRAG = `#version 300 es
precision mediump float;
uniform vec4 u_color;
out vec4 color;
void main() {
  color = u_color;
}`;

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

export class TraceRenderer {
  private gl: WebGL2RenderingContext;
  private prog: WebGLProgram;
  private vao: WebGLVertexArrayObject;
  private xBuf: WebGLBuffer;
  private yBuf: WebGLBuffer;
  private uX: WebGLUniformLocation;
  private uY: WebGLUniformLocation;
  private uColor: WebGLUniformLocation;
  /** [start, count] of each run without NaN */
  private runs: [number, number][] = [];

  constructor(private canvas: HTMLCanvasElement) {
    const gl = canvas.getContext('webgl2', { antialias: true, alpha: true, premultipliedAlpha: true });
    if (!gl) throw new Error('WebGL2 is not available');
    this.gl = gl;
    this.prog = link(gl, VERT, FRAG);
    this.vao = gl.createVertexArray()!;
    this.xBuf = gl.createBuffer()!;
    this.yBuf = gl.createBuffer()!;
    gl.bindVertexArray(this.vao);
    for (const [buf, name] of [
      [this.xBuf, 'a_x'],
      [this.yBuf, 'a_y'],
    ] as const) {
      const loc = gl.getAttribLocation(this.prog, name);
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 1, gl.FLOAT, false, 0, 0);
    }
    gl.bindVertexArray(null);
    this.uX = gl.getUniformLocation(this.prog, 'u_x')!;
    this.uY = gl.getUniformLocation(this.prog, 'u_y')!;
    this.uColor = gl.getUniformLocation(this.prog, 'u_color')!;
  }

  /** Matches the drawing buffer to the canvas's size in device pixels. */
  resize(): void {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(this.canvas.clientWidth * dpr));
    const h = Math.max(1, Math.round(this.canvas.clientHeight * dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
  }

  setTrace(x: Float32Array, y: Float32Array): void {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.xBuf);
    gl.bufferData(gl.ARRAY_BUFFER, x, gl.DYNAMIC_DRAW);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.yBuf);
    gl.bufferData(gl.ARRAY_BUFFER, y, gl.DYNAMIC_DRAW);
    this.runs = [];
    let start = -1;
    for (let i = 0; i <= y.length; i++) {
      const gap = i === y.length || Number.isNaN(y[i]);
      if (!gap && start < 0) start = i;
      if (gap && start >= 0) {
        this.runs.push([start, i - start]);
        start = -1;
      }
    }
  }

  /** Draws into `rect` (CSS pixels from the top left); clip space spans the rect. */
  draw(x: Axis, y: Axis, rgba: [number, number, number, number], rect: Rect): void {
    const gl = this.gl;
    const dpr = this.canvas.width / Math.max(1, this.canvas.clientWidth);
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    if (!this.runs.length) return;
    const [vx, vw, vh] = [Math.round(rect.x * dpr), Math.round(rect.w * dpr), Math.round(rect.h * dpr)];
    const vy = this.canvas.height - Math.round(rect.y * dpr) - vh;
    gl.viewport(vx, vy, vw, vh);
    gl.enable(gl.SCISSOR_TEST);
    gl.scissor(vx, vy, vw, vh);
    gl.useProgram(this.prog);
    gl.uniform2f(this.uX, x.a, x.b);
    gl.uniform2f(this.uY, y.a, y.b);
    gl.uniform4f(this.uColor, ...rgba);
    gl.bindVertexArray(this.vao);
    for (const [start, count] of this.runs) {
      gl.drawArrays(count === 1 ? gl.POINTS : gl.LINE_STRIP, start, count);
    }
    gl.bindVertexArray(null);
    gl.disable(gl.SCISSOR_TEST);
  }

  clear(): void {
    this.runs = [];
    this.gl.clearColor(0, 0, 0, 0);
    this.gl.clear(this.gl.COLOR_BUFFER_BIT);
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
