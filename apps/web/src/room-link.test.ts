import { describe, expect, it } from "vitest";
import { normaliseCode, roomFromSearch, roomLink, roomSearch } from "./room-link";

describe("room codes in the page's URL", () => {
  it("reads the code a shared link carries, as typed", () => {
    expect(roomFromSearch("?room=BXQF")).toBe("BXQF");
    expect(roomFromSearch("?track=oval&room=bxqf")).toBe("BXQF");
    expect(roomFromSearch("")).toBeNull();
    expect(roomFromSearch("?room=")).toBeNull();
  });

  it("builds the link to share from the page's origin and the code", () => {
    const location = { origin: "https://formulanone.sushimafia.com", pathname: "/" };
    expect(roomLink(location, "BXQF")).toBe("https://formulanone.sushimafia.com/?room=BXQF");
    expect(roomSearch("BXQF")).toBe("?room=BXQF");
  });

  it("normalises a typed code to the server's form: upper case, no spaces", () => {
    expect(normaliseCode(" bx qf ")).toBe("BXQF");
    expect(normaliseCode("")).toBe("");
  });
});
