sub init()
    m.player = m.top.FindNode("player")
    m.player.ObserveField("state", "onState")
    m.from = "your phone"
end sub

sub onCommand()
    c = m.top.command
    if c = invalid then return
    if c.DoesExist("from") then m.from = c.from
    if c.DoesExist("url") and c.url <> "" then
        content = CreateObject("roSGNode", "ContentNode")
        content.url = c.url
        content.title = Text(c, "title")
        m.top.FindNode("title").text = Text(c, "title")
        m.top.FindNode("artist").text = Text(c, "artist")
        m.top.FindNode("album").text = Text(c, "album")
        if c.DoesExist("art") then m.top.FindNode("cover").uri = c.art
        m.player.content = content
        m.player.control = "play"
        if c.DoesExist("pos") then
            ms = Val(c.pos)
            if ms > 0 then m.player.seek = ms / 1000
        end if
        return
    end if
    action = Text(c, "action")
    if action = "play" then
        m.player.control = "resume"
    else if action = "pause" then
        m.player.control = "pause"
    else if action = "stop" then
        m.player.control = "stop"
        m.top.FindNode("status").text = "The music went back to " + m.from
    else if action = "seek" and c.DoesExist("pos") then
        m.player.seek = Val(c.pos) / 1000
    end if
end sub

sub onState()
    s = m.player.state
    status = m.top.FindNode("status")
    if s = "playing" then
        status.text = "From " + m.from
    else if s = "paused" then
        status.text = "Paused · from " + m.from
    else if s = "error" then
        status.text = "This song couldn't play on the TV"
    else if s = "finished" then
        status.text = "From " + m.from
    end if
end sub

function Text(c as Object, k as String) as String
    if c.DoesExist(k) and c[k] <> invalid then return c[k]
    return ""
end function

' The Roku remote's own keys: play/pause toggles; Back leaves the channel as usual.
function onKeyEvent(key as String, press as Boolean) as Boolean
    if not press then return false
    if key = "play" then
        if m.player.state = "playing" then m.player.control = "pause" else m.player.control = "resume"
        return true
    end if
    return false
end function
