def markdown(path = None, id = None, title = None, area = None, content = None, source = None, resource = None):
    if id == None and title == None and area == None and content == None and source == None and resource == None:
        return {"path": path}
    return component(
        "doc.markdown",
        id = id,
        title = title,
        area = area,
        props = _without_empty({
            "path": path,
            "content": content,
            "source": source,
            "resource": resource,
        }),
    )

def markdown_ref(path, id = None, title = None, area = None, source = None, resource = None):
    return markdown(
        path = path,
        id = id,
        title = title,
        area = area,
        source = source,
        resource = resource,
    )
