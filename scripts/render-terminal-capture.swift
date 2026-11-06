// Rasterize the real dashboard's parsed PTY cell capture (not a native-terminal screenshot).
import AppKit
import Foundation
let source = URL(fileURLWithPath: CommandLine.arguments[1])
let data = try Data(contentsOf: source)
let capture = try JSONSerialization.jsonObject(with: data) as! [String: Any]
let cols = capture["cols"] as! Int, rows = capture["rows"] as! Int
let cw = 10, ch = 20
let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: cols*cw, pixelsHigh: rows*ch, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
let context = NSGraphicsContext(bitmapImageRep: bitmap)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = context
NSColor(red:12/255,green:17/255,blue:24/255,alpha:1).setFill()
NSRect(x:0,y:0,width:cols*cw,height:rows*ch).fill()
func color(_ value: String, _ background: Bool) -> NSColor {
    let palette: [(Int,Int,Int)] = [(0,0,0),(205,49,49),(13,188,121),(229,229,16),(36,114,200),(188,63,188),(17,168,205),(229,229,229),(102,102,102),(241,76,76),(35,209,139),(245,245,67),(59,142,234),(214,112,214),(41,184,219),(255,255,255)]
    let numbers = value.split(whereSeparator: { !$0.isNumber }).compactMap { Int($0) }
    var rgb = background ? (12,17,24) : (205,214,224)
    if value.hasPrefix("Rgb"), numbers.count == 3 { rgb = (numbers[0], numbers[1], numbers[2]) }
    if value.hasPrefix("Idx"), let n = numbers.first, n < 16 { rgb = palette[n] }
    return NSColor(red: CGFloat(rgb.0)/255, green: CGFloat(rgb.1)/255, blue: CGFloat(rgb.2)/255, alpha: 1)
}
for cell in capture["cells"] as! [[String:Any]] {
    let x = (cell["col"] as! Int)*cw, y = (rows-1-(cell["row"] as! Int))*ch
    color(cell["bg"] as! String, true).setFill()
    NSRect(x:x,y:y,width:cw,height:ch).fill()
}
for cell in capture["cells"] as! [[String:Any]] where !(cell["wide"] as! Bool) {
    let x = (cell["col"] as! Int)*cw, y = (rows-1-(cell["row"] as! Int))*ch
    let font = NSFont(name: (cell["bold"] as! Bool) ? "Menlo-Bold" : "Menlo-Regular", size: 15)!
    (cell["text"] as! String).draw(at:NSPoint(x:x,y:y+1),withAttributes:[.font:font,.foregroundColor:color(cell["fg"] as! String,false)])
}
if !(capture["hide_cursor"] as? Bool ?? true), let cursor = capture["cursor"] as? [Int] {
    NSColor.white.setStroke()
    NSBezierPath(rect:NSRect(x:cursor[1]*cw,y:(rows-1-cursor[0])*ch,width:cw,height:ch)).stroke()
}
NSGraphicsContext.restoreGraphicsState()
try bitmap.representation(using:.png,properties:[:])!.write(to:source.deletingPathExtension().appendingPathExtension("png"))
