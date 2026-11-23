import Foundation

// small regex helpers for the weebcentral parser
// the site serves plain html fragments, targeted patterns are lighter than a full html parser

enum RegexCache {
    private static let cache = NSCache<NSString, NSRegularExpression>()

    static func regex(_ pattern: String) -> NSRegularExpression? {
        let key = pattern as NSString
        if let cached = cache.object(forKey: key) {
            return cached
        }
        guard let regex = try? NSRegularExpression(pattern: pattern, options: []) else {
            return nil
        }
        cache.setObject(regex, forKey: key)
        return regex
    }
}

// nsregularexpression works on utf-16, copying into an nsstring once is much faster
// than bridging the string on every match (chapter lists can be over 2 mb)
struct RegexText {
    private let text: NSString

    init(_ string: String) {
        text = NSString(string: string)
    }

    private var fullRange: NSRange {
        NSRange(location: 0, length: text.length)
    }

    func first(_ pattern: String) -> String? {
        guard let regex = RegexCache.regex(pattern),
              let match = regex.firstMatch(in: text as String, options: [], range: fullRange),
              match.numberOfRanges > 1
        else { return nil }
        let range = match.range(at: 1)
        guard range.location != NSNotFound else { return nil }
        return text.substring(with: range)
    }

    // capture groups without group 0
    func all(_ pattern: String) -> [[String]] {
        guard let regex = RegexCache.regex(pattern) else { return [] }
        return regex.matches(in: text as String, options: [], range: fullRange).map { match in
            (1..<match.numberOfRanges).map { index in
                let range = match.range(at: index)
                return range.location == NSNotFound ? "" : text.substring(with: range)
            }
        }
    }
}

extension String {
    func firstCapture(_ pattern: String) -> String? {
        RegexText(self).first(pattern)
    }

    func allCaptures(_ pattern: String) -> [[String]] {
        RegexText(self).all(pattern)
    }

    var htmlDecoded: String {
        guard contains("&") else { return self }
        var text = self

        if let regex = RegexCache.regex(#"&#(x?)([0-9a-fA-F]+);"#) {
            let matches = regex.matches(in: text, options: [], range: NSRange(text.startIndex..<text.endIndex, in: text))
            if !matches.isEmpty {
                var output = ""
                var cursor = text.startIndex
                for match in matches {
                    guard let whole = Range(match.range, in: text),
                          let hexFlag = Range(match.range(at: 1), in: text),
                          let digits = Range(match.range(at: 2), in: text)
                    else { continue }
                    output += text[cursor..<whole.lowerBound]
                    let radix = text[hexFlag].isEmpty ? 10 : 16
                    if let code = UInt32(text[digits], radix: radix), let scalar = Unicode.Scalar(code) {
                        output.unicodeScalars.append(scalar)
                    } else {
                        output += text[whole]
                    }
                    cursor = whole.upperBound
                }
                output += text[cursor..<text.endIndex]
                text = output
            }
        }

        let named: [(String, String)] = [
            ("&quot;", "\""),
            ("&apos;", "'"),
            ("&lt;", "<"),
            ("&gt;", ">"),
            ("&nbsp;", " "),
            ("&amp;", "&"),
        ]
        for (entity, value) in named {
            text = text.replacingOccurrences(of: entity, with: value)
        }
        return text
    }

    // decoded, trimmed and with whitespace collapsed
    var cleanedText: String {
        htmlDecoded
            .components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}
