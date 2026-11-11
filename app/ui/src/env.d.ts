// Types for .svelte imports in .ts files (svelte-check types the components themselves).
declare module '*.svelte' {
  import type { Component } from 'svelte';
  const component: Component;
  export default component;
}

/** True in the viewer builds (an HTML export): editing, importing and exporting dialogs are left out of the bundle. */
declare const __VIEWER__: boolean;
