' Encore for Roku. The phone starts the channel with
'   POST http://<roku>:8060/launch/dev?url=<relay address>&title=..&artist=..&album=..&art=..&pos=<ms>
' and drives it with
'   POST http://<roku>:8060/input?action=play|pause|stop|seek&pos=<ms>   or a new url=...
' It reads the playing state back with GET /query/media-player (Roku's own report).
' Addresses come only from the phone's relay: a per-session pass that can only play
' the queue it was made for (ADR android-0007). Nothing is stored on the Roku.

sub Main(args as Dynamic)
    screen = CreateObject("roSGScreen")
    port = CreateObject("roMessagePort")
    screen.SetMessagePort(port)
    scene = screen.CreateScene("MainScene")
    screen.Show()

    input = CreateObject("roInput")
    input.SetMessagePort(port)

    if args <> invalid then scene.command = Command(args)

    while true
        msg = wait(0, port)
        t = type(msg)
        if t = "roSGScreenEvent"
            if msg.IsScreenClosed() then return
        else if t = "roInputEvent"
            if msg.IsInput() then scene.command = Command(msg.GetInfo())
        end if
    end while
end sub

' Only the fields the phone sends, as plain strings; anything else is dropped.
function Command(a as Object) as Object
    c = {}
    for each k in ["url", "title", "artist", "album", "art", "pos", "action", "from"]
        if a.DoesExist(k) and a[k] <> invalid then c[k] = a[k].ToStr()
    end for
    return c
end function
