# 内置正文文本：text("…") 等价于 component("mei.text", props={content: …})

def text(content = None, area = None, id = None, html = None, format = None, resource = None, font = None, align = None):
    props = {}
    if content != None:
        props["content"] = content
    if html != None:
        props["html"] = html
    if format != None:
        props["format"] = format
    if resource != None:
        props["resource"] = resource
    if font != None:
        props["font"] = font
    if align != None:
        props["align"] = align
    return component(
        "mei.text",
        id = id,
        area = area,
        props = _without_empty(props),
    )
