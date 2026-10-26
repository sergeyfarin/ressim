/** The two supported static hosting layouts. Keep the live subpath as the default. */
export function buildLayout(basePath = process.env.RESIM_BASE_PATH ?? '/ressim/') {
  if (basePath !== '/ressim/' && basePath !== '/') {
    throw new Error('RESIM_BASE_PATH must be /ressim/ or /')
  }

  const root = basePath === '/'
  return {
    basePath,
    // Separate outputs let both candidates be validated without overwriting each other.
    outDir: root ? 'dist-root' : 'dist/ressim',
    archiveParent: root ? 'dist-root' : 'dist',
    archiveEntry: root ? '.' : 'ressim',
    archivePrefix: root ? 'ressim-root-dist' : 'ressim-dist',
    metadataFile: root ? 'dist-root-build-meta.json' : 'dist-build-meta.json',
  }
}
