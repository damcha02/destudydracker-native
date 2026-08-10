import { useEffect, useRef, useState } from "react";

type WabiRestFluidRingProps = {
  progress: number;
  running: boolean;
  dark: boolean;
};

const VERTEX_SHADER = `#version 300 es
in vec2 position;
out vec2 uv;
void main() {
  uv = position * .5 + .5;
  gl_Position = vec4(position, 0., 1.);
}`;

const ADVECTION_SHADER = `#version 300 es
precision highp float;
in vec2 uv;
out vec4 color;
uniform sampler2D source;
uniform sampler2D velocity;
uniform vec2 texel;
uniform float dt;
uniform float dissipation;
void main() {
  vec2 point = uv - dt * texture(velocity, uv).xy * texel * 22.;
  color = texture(source, clamp(point, vec2(.001), vec2(.999))) * dissipation;
}`;

const FORCE_SHADER = `#version 300 es
precision highp float;
in vec2 uv;
out vec4 color;
uniform sampler2D velocity;
uniform float time;
uniform float dt;
uniform float fill;
void main() {
  vec2 flow = texture(velocity, uv).xy;
  vec2 swirl = vec2(
    sin(uv.y * 12. + time * .68) + cos(uv.y * 7. - time * .31),
    sin(uv.x * 10. - time * .42) * .42
  );
  float edge = smoothstep(.04, .24, uv.x) * smoothstep(.96, .76, uv.x);
  flow += swirl * edge * dt * .22;
  float inlet = smoothstep(min(.18, fill + .02), .02, uv.y) * smoothstep(.001, .025, fill);
  float ceiling = smoothstep(fill + .02, fill + .13, uv.y);
  flow.y += inlet * dt * 1.5;
  flow.y -= ceiling * dt * .45;
  color = vec4(flow * .991, 0., 1.);
}`;

const DIVERGENCE_SHADER = `#version 300 es
precision highp float;
in vec2 uv;
out vec4 color;
uniform sampler2D velocity;
uniform vec2 texel;
void main() {
  float l = texture(velocity, uv - vec2(texel.x, 0.)).x;
  float r = texture(velocity, uv + vec2(texel.x, 0.)).x;
  float b = texture(velocity, uv - vec2(0., texel.y)).y;
  float t = texture(velocity, uv + vec2(0., texel.y)).y;
  color = vec4((r - l + t - b) * .5, 0., 0., 1.);
}`;

const PRESSURE_SHADER = `#version 300 es
precision highp float;
in vec2 uv;
out vec4 color;
uniform sampler2D pressure;
uniform sampler2D divergence;
uniform vec2 texel;
void main() {
  float l = texture(pressure, uv - vec2(texel.x, 0.)).x;
  float r = texture(pressure, uv + vec2(texel.x, 0.)).x;
  float b = texture(pressure, uv - vec2(0., texel.y)).x;
  float t = texture(pressure, uv + vec2(0., texel.y)).x;
  float d = texture(divergence, uv).x;
  color = vec4((l + r + b + t - d) * .25, 0., 0., 1.);
}`;

const GRADIENT_SHADER = `#version 300 es
precision highp float;
in vec2 uv;
out vec4 color;
uniform sampler2D velocity;
uniform sampler2D pressure;
uniform vec2 texel;
void main() {
  float l = texture(pressure, uv - vec2(texel.x, 0.)).x;
  float r = texture(pressure, uv + vec2(texel.x, 0.)).x;
  float b = texture(pressure, uv - vec2(0., texel.y)).x;
  float t = texture(pressure, uv + vec2(0., texel.y)).x;
  vec2 flow = texture(velocity, uv).xy - vec2(r - l, t - b) * .5;
  color = vec4(flow, 0., 1.);
}`;

const DENSITY_SHADER = `#version 300 es
precision highp float;
in vec2 uv;
out vec4 color;
uniform sampler2D density;
uniform sampler2D velocity;
uniform vec2 texel;
uniform float dt;
uniform float fill;
uniform float time;
void main() {
  vec2 flow = texture(velocity, uv).xy;
  vec2 point = uv - dt * flow * texel * 22.;
  float carried = texture(density, clamp(point, vec2(.001), vec2(.999))).x * .996;
  float ripple = sin(uv.x * 19. + time * .8) * .008 + sin(uv.x * 7. - time * .43) * .012;
  float fillHeight = clamp(fill, 0., 1.);
  float inlet = smoothstep(min(.15, fillHeight + .02), .015, uv.y) * smoothstep(.001, .025, fillHeight);
  float ceiling = 1. - smoothstep(fillHeight + .045 + ripple, fillHeight + .13 + ripple, uv.y);
  float ink = fillHeight < .001 ? 0. : max(carried * .998, inlet) * ceiling;
  color = vec4(ink, 0., 0., 1.);
}`;

const DISPLAY_SHADER = `#version 300 es
precision highp float;
in vec2 uv;
out vec4 color;
uniform sampler2D density;
uniform sampler2D velocity;
uniform float lightInk;
void main() {
  float radius = length(uv - .5);
  float vessel = 1. - smoothstep(.447, .456, radius);
  float ink = smoothstep(.08, .52, texture(density, uv).x) * vessel;
  vec2 flow = texture(velocity, uv).xy;
  float wash = smoothstep(.18, .8, texture(density, uv + flow * .012).x) * .12;
  vec3 blackInk = vec3(.035, .035, .025) + wash;
  vec3 paperInk = vec3(.91, .89, .81) + wash * .55;
  color = vec4(mix(blackInk, paperInk, lightInk), ink);
}`;

function compileProgram(gl: WebGL2RenderingContext, fragmentSource: string) {
  const compile = (type: number, source: string) => {
    const shader = gl.createShader(type);
    if (!shader) throw new Error("Could not create WebGL shader.");
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) ?? "WebGL shader compilation failed.");
    return shader;
  };
  const program = gl.createProgram();
  if (!program) throw new Error("Could not create WebGL program.");
  gl.attachShader(program, compile(gl.VERTEX_SHADER, VERTEX_SHADER));
  gl.attachShader(program, compile(gl.FRAGMENT_SHADER, fragmentSource));
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) ?? "WebGL program linking failed.");
  return program;
}

function WabiRestRingFallback({ progress, running, dark }: WabiRestFluidRingProps) {
  const liquidOffset = 19 - progress * 90;
  return <svg className={`wabi-rest-ring ${dark ? "is-light-ink" : ""} ${running ? "is-running" : "is-paused"}`} viewBox="0 0 100 100" role="img" aria-label={`${Math.round(progress * 100)} percent of the break elapsed`}>
    <defs><clipPath id="wabi-rest-ink-clip"><circle cx="50" cy="50" r="45" /></clipPath></defs>
    <g clipPath="url(#wabi-rest-ink-clip)">
      <g transform={`translate(0 ${liquidOffset})`}><path className="wabi-rest-liquid wabi-rest-liquid--base" d="M-8 76 C8 71 19 80 33 75 C47 70 60 80 73 74 C86 68 99 79 108 73 L108 110 L-8 110Z" /></g>
      <g transform={`translate(0 ${liquidOffset + 2})`}><path className="wabi-rest-liquid wabi-rest-liquid--wash" d="M-8 78 C9 73 20 82 35 77 C50 72 61 82 76 76 C89 71 100 80 108 75 L108 110 L-8 110Z" /></g>
    </g>
    <circle className="wabi-rest-outline" cx="50" cy="50" r="45" />
  </svg>;
}

export function WabiRestFluidRing({ progress, running, dark }: WabiRestFluidRingProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const latest = useRef({ progress, running, dark });
  const [useFallback, setUseFallback] = useState(false);

  useEffect(() => {
    latest.current = { progress, running, dark };
  }, [progress, running, dark]);

  useEffect(() => {
    canvasRef.current?.dispatchEvent(new Event("wabi-fluid-wake"));
  }, [progress, running, dark]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setUseFallback(true);
      return;
    }

    const gl = canvas.getContext("webgl2", { alpha: true, antialias: false, powerPreference: "low-power" });
    if (!gl || !gl.getExtension("EXT_color_buffer_float")) {
      setUseFallback(true);
      return;
    }

    let animationFrame: number | null = null;
    let lastFrame = 0;
    let failed = false;
    let hasRendered = false;
    let renderRequested = true;
    const size = 128;
    const textureOptions = { internalFormat: gl.RGBA16F, format: gl.RGBA, type: gl.HALF_FLOAT };
    const textures: WebGLTexture[] = [];
    const framebuffers: WebGLFramebuffer[] = [];
    const programs: WebGLProgram[] = [];
    const createTexture = () => {
      const texture = gl.createTexture();
      if (!texture) throw new Error("Could not create fluid texture.");
      textures.push(texture);
      gl.bindTexture(gl.TEXTURE_2D, texture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texImage2D(gl.TEXTURE_2D, 0, textureOptions.internalFormat, size, size, 0, textureOptions.format, textureOptions.type, null);
      return texture;
    };
    const createFramebuffer = (texture: WebGLTexture) => {
      const framebuffer = gl.createFramebuffer();
      if (!framebuffer) throw new Error("Could not create fluid framebuffer.");
      framebuffers.push(framebuffer);
      gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texture, 0);
      if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) throw new Error("Fluid framebuffer is incomplete.");
      return framebuffer;
    };

    try {
      const quad = gl.createBuffer();
      if (!quad) throw new Error("Could not create fluid geometry.");
      gl.bindBuffer(gl.ARRAY_BUFFER, quad);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
      const velocity = [{ texture: createTexture(), framebuffer: null as WebGLFramebuffer | null }, { texture: createTexture(), framebuffer: null as WebGLFramebuffer | null }];
      const density = [{ texture: createTexture(), framebuffer: null as WebGLFramebuffer | null }, { texture: createTexture(), framebuffer: null as WebGLFramebuffer | null }];
      const pressure = [{ texture: createTexture(), framebuffer: null as WebGLFramebuffer | null }, { texture: createTexture(), framebuffer: null as WebGLFramebuffer | null }];
      velocity.forEach((target) => { target.framebuffer = createFramebuffer(target.texture); });
      density.forEach((target) => { target.framebuffer = createFramebuffer(target.texture); });
      pressure.forEach((target) => { target.framebuffer = createFramebuffer(target.texture); });
      const divergenceTexture = createTexture();
      const divergenceFramebuffer = createFramebuffer(divergenceTexture);
      gl.clearColor(0, 0, 0, 0);
      framebuffers.forEach((framebuffer) => {
        gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
        gl.clear(gl.COLOR_BUFFER_BIT);
      });
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      const advect = compileProgram(gl, ADVECTION_SHADER);
      const force = compileProgram(gl, FORCE_SHADER);
      const divergence = compileProgram(gl, DIVERGENCE_SHADER);
      const pressureProgram = compileProgram(gl, PRESSURE_SHADER);
      const gradient = compileProgram(gl, GRADIENT_SHADER);
      const densityProgram = compileProgram(gl, DENSITY_SHADER);
      const display = compileProgram(gl, DISPLAY_SHADER);
      programs.push(advect, force, divergence, pressureProgram, gradient, densityProgram, display);
      let velocityRead = 0;
      let densityRead = 0;
      let pressureRead = 0;

      const draw = (program: WebGLProgram, target: WebGLFramebuffer | null, uniforms: Record<string, number | WebGLTexture | [number, number]>) => {
        gl.useProgram(program);
        gl.bindFramebuffer(gl.FRAMEBUFFER, target);
        gl.viewport(0, 0, target ? size : canvas.width, target ? size : canvas.height);
        const position = gl.getAttribLocation(program, "position");
        gl.enableVertexAttribArray(position);
        gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
        let unit = 0;
        Object.entries(uniforms).forEach(([name, value]) => {
          const location = gl.getUniformLocation(program, name);
          if (!location) return;
          if (value instanceof WebGLTexture) {
            gl.activeTexture(gl.TEXTURE0 + unit);
            gl.bindTexture(gl.TEXTURE_2D, value);
            gl.uniform1i(location, unit++);
          } else if (Array.isArray(value)) gl.uniform2f(location, value[0], value[1]);
          else gl.uniform1f(location, value);
        });
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      };
      const swapVelocity = () => { velocityRead = 1 - velocityRead; };
      const swapDensity = () => { densityRead = 1 - densityRead; };
      const swapPressure = () => { pressureRead = 1 - pressureRead; };
      const requestNextFrame = () => {
        if (!failed && animationFrame === null && !document.hidden && latest.current.running) animationFrame = requestAnimationFrame(frame);
      };
      const frame = (now: number) => {
        animationFrame = null;
        if (failed) return;
        const { progress: currentProgress, running: currentRunning, dark: currentDark } = latest.current;
        const shouldSimulate = currentRunning && !document.hidden;
        if (hasRendered && !shouldSimulate && !renderRequested) return;
        if (shouldSimulate && now - lastFrame < 33) {
          requestNextFrame();
          return;
        }
        const dt = Math.min((now - lastFrame) / 1000, 0.04) || 0.033;
        lastFrame = now;
        const texel: [number, number] = [1 / size, 1 / size];
        const velocityWrite = 1 - velocityRead;
        draw(advect, velocity[velocityWrite].framebuffer, { source: velocity[velocityRead].texture, velocity: velocity[velocityRead].texture, texel, dt, dissipation: 0.993 });
        velocityRead = velocityWrite;
        draw(force, velocity[1 - velocityRead].framebuffer, { velocity: velocity[velocityRead].texture, time: now / 1000, dt, fill: currentProgress });
        swapVelocity();
        draw(divergence, divergenceFramebuffer, { velocity: velocity[velocityRead].texture, texel });
        for (let i = 0; i < 5; i += 1) {
          draw(pressureProgram, pressure[1 - pressureRead].framebuffer, { pressure: pressure[pressureRead].texture, divergence: divergenceTexture, texel });
          swapPressure();
        }
        draw(gradient, velocity[1 - velocityRead].framebuffer, { velocity: velocity[velocityRead].texture, pressure: pressure[pressureRead].texture, texel });
        swapVelocity();
        draw(densityProgram, density[1 - densityRead].framebuffer, { density: density[densityRead].texture, velocity: velocity[velocityRead].texture, texel, dt, fill: currentProgress, time: now / 1000 });
        swapDensity();
        draw(display, null, { density: density[densityRead].texture, velocity: velocity[velocityRead].texture, lightInk: currentDark ? 1 : 0 });
        hasRendered = true;
        renderRequested = false;
        if (shouldSimulate) requestNextFrame();
      };
      const wake = () => {
        renderRequested = true;
        if (!failed && animationFrame === null && !document.hidden) animationFrame = requestAnimationFrame(frame);
      };
      const onVisibilityChange = () => { if (!document.hidden) wake(); };
      const onContextLost = (event: Event) => { event.preventDefault(); failed = true; if (animationFrame !== null) cancelAnimationFrame(animationFrame); setUseFallback(true); };
      canvas.addEventListener("webglcontextlost", onContextLost);
      canvas.addEventListener("wabi-fluid-wake", wake);
      document.addEventListener("visibilitychange", onVisibilityChange);
      animationFrame = requestAnimationFrame(frame);
      return () => {
        failed = true;
        canvas.removeEventListener("webglcontextlost", onContextLost);
        canvas.removeEventListener("wabi-fluid-wake", wake);
        document.removeEventListener("visibilitychange", onVisibilityChange);
        if (animationFrame !== null) cancelAnimationFrame(animationFrame);
        textures.forEach((texture) => gl.deleteTexture(texture));
        framebuffers.forEach((framebuffer) => gl.deleteFramebuffer(framebuffer));
        programs.forEach((program) => gl.deleteProgram(program));
        gl.deleteBuffer(quad);
      };
    } catch {
      setUseFallback(true);
    }
  }, []);

  if (useFallback) return <WabiRestRingFallback progress={progress} running={running} dark={dark} />;
  return <canvas ref={canvasRef} className={`wabi-rest-ring wabi-rest-fluid-ring ${dark ? "is-light-ink" : ""}`} width="192" height="192" role="img" aria-label={`${Math.round(progress * 100)} percent of the break elapsed`} />;
}
