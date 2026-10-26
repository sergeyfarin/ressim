import { readFile, copyFile, appendFile } from 'node:fs/promises'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { buildLayout } from './build-layout.ts'

const layout = buildLayout()
// In CI the workflow revision is authoritative, even if a build override was supplied.
const expectedSha = process.env.GITHUB_SHA ?? process.env.RESIM_BUILD_SHA
if (!expectedSha || !/^[a-f0-9]{40}$/.test(expectedSha)) {
  throw new Error('Packaging requires a full RESIM_BUILD_SHA or GITHUB_SHA')
}
const metadataPath = `${layout.outDir}/build-info.json`
const metadata = JSON.parse(await readFile(metadataPath, 'utf8'))
if (metadata.commit !== expectedSha || metadata.basePath !== layout.basePath) {
  throw new Error('Build SHA or base path mismatch; rebuild before packaging')
}

const archive = `${layout.archivePrefix}-${expectedSha}.tar.gz`
await promisify(execFile)('tar', ['-czf', archive, '-C', layout.archiveParent, layout.archiveEntry])
await copyFile(metadataPath, layout.metadataFile)
if (process.env.GITHUB_OUTPUT) {
  await appendFile(process.env.GITHUB_OUTPUT,
    `app_dir=${layout.outDir}\narchive=${archive}\nmetadata_file=${layout.metadataFile}\n`)
}
console.log(`Packaged ${archive} and ${layout.metadataFile}`)
