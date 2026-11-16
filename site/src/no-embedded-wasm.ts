// The site passes its own .wasm URL to initText(), so the package's embedded
// copy is never loaded. vite.config.ts aliases it to this stub to keep it out
// of the build.
export default '';
