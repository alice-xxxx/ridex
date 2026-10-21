export function displayFileName(path: string): string {
  let decoded = path
  try {
    decoded = decodeURIComponent(path)
  } catch {
    // Keep malformed path text usable.
  }
  return decoded.split(/[\\/]/).pop() || decoded
}
