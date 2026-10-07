/** Regional-indicator pair for a two letter country code. */
const ri = (cc: string) => String.fromCodePoint(...[...cc].map((c) => 0x1f1e6 + c.charCodeAt(0) - 65))

/** The countries the speed endpoints and the exit lookup report. */
const CC: Record<string, string> = {
  italy: "IT", germany: "DE", netherlands: "NL", france: "FR", spain: "ES",
  portugal: "PT", switzerland: "CH", austria: "AT", belgium: "BE", ireland: "IE",
  "united kingdom": "GB", uk: "GB", england: "GB", "united states": "US",
  usa: "US", canada: "CA", mexico: "MX", brazil: "BR", argentina: "AR",
  chile: "CL", colombia: "CO", sweden: "SE", norway: "NO", denmark: "DK",
  finland: "FI", poland: "PL", czechia: "CZ", "czech republic": "CZ",
  romania: "RO", hungary: "HU", greece: "GR", ukraine: "UA", turkey: "TR",
  russia: "RU", egypt: "EG", "south africa": "ZA", nigeria: "NG", kenya: "KE",
  morocco: "MA", "saudi arabia": "SA", "united arab emirates": "AE",
  uae: "AE", qatar: "QA", israel: "IL", india: "IN", singapore: "SG",
  japan: "JP", "south korea": "KR", china: "CN", "hong kong": "HK",
  taiwan: "TW", indonesia: "ID", malaysia: "MY", thailand: "TH",
  vietnam: "VN", philippines: "PH", australia: "AU", "new zealand": "NZ",
}

/** "Milan, Italy" -> "flag Milan, Italy". Empty prefix when unknown. */
export function flagFor(place: string): string {
  const country = place.split(",").pop()?.trim().toLowerCase() ?? ""
  const cc = CC[country]
  return cc ? ri(cc) + " " : ""
}
