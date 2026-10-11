// @ts-nocheck
// Pandora addition, not part of foliate-js. The upstream pdf.js pulls in pdf.js from
// Mozilla (13 MB). PDF is not supported yet, so this stands in for it.
export const makePDF = async () => {
    throw new Error('PDF is not supported')
}
