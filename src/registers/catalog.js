import catalog from "./components/vcu.json" with { type: "json" }

export { catalog }

export const readBlocks = catalog.readBlocks
export const coilReadBlocks = catalog.coilReadBlocks
export const categories = catalog.categories
export const registerDefinitions = catalog.registers
export const coilDefinitions = catalog.coils

export function findRegisterBlock(address) {
    const numericAddress = Number(address)
    return readBlocks.find(
        ({ start, count }) => numericAddress >= start && numericAddress < start + count,
    ) ?? null
}

export function findCoilBlock(address) {
    const numericAddress = Number(address)
    return coilReadBlocks.find(
        ({ start, count }) => numericAddress >= start && numericAddress < start + count,
    ) ?? null
}
