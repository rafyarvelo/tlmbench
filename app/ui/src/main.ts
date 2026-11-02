// Mounts the window, then starts the app (app.ts finds its elements by id).

import { mount } from 'svelte';

import App from './App.svelte';

mount(App, { target: document.body });
await import('./app');
