// Types for .svelte imports in .ts files (svelte-check types the components themselves).
declare module '*.svelte' {
  import type { Component } from 'svelte';
  const component: Component;
  export default component;
}
