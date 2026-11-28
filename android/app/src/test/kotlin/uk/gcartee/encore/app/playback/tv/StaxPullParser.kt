package uk.gcartee.encore.app.playback.tv

import java.io.InputStream
import java.io.Reader
import javax.xml.stream.XMLInputFactory
import javax.xml.stream.XMLStreamConstants
import javax.xml.stream.XMLStreamReader
import org.xmlpull.v1.XmlPullParser

/**
 * The part of the platform's XmlPullParser that Dlna.kt uses, on the JVM's own StAX reader, so
 * the DLNA code is tested on CI's JVM without Android (whose parser is a stub there) and
 * without a library. DTDs and external entities are off, as on the phone.
 */
class StaxPullParser : XmlPullParser {
    private var reader: XMLStreamReader? = null
    private var event = XmlPullParser.START_DOCUMENT

    override fun setFeature(name: String?, state: Boolean) = Unit
    override fun getFeature(name: String?): Boolean = name == XmlPullParser.FEATURE_PROCESS_NAMESPACES
    override fun setProperty(name: String?, value: Any?) = Unit
    override fun getProperty(name: String?): Any? = null

    override fun setInput(input: Reader?) {
        val factory = XMLInputFactory.newInstance()
        factory.setProperty(XMLInputFactory.SUPPORT_DTD, false)
        factory.setProperty(XMLInputFactory.IS_SUPPORTING_EXTERNAL_ENTITIES, false)
        factory.setProperty(XMLInputFactory.IS_NAMESPACE_AWARE, true)
        factory.setProperty(XMLInputFactory.IS_COALESCING, true)
        reader = factory.createXMLStreamReader(input)
        event = XmlPullParser.START_DOCUMENT
    }

    override fun setInput(inputStream: InputStream?, inputEncoding: String?) = setInput(inputStream?.reader(charset(inputEncoding ?: "UTF-8")))

    override fun next(): Int {
        val r = reader ?: throw IllegalStateException("no input")
        while (r.hasNext()) {
            when (r.next()) {
                XMLStreamConstants.START_ELEMENT -> return XmlPullParser.START_TAG.also { event = it }
                XMLStreamConstants.END_ELEMENT -> return XmlPullParser.END_TAG.also { event = it }
                XMLStreamConstants.CHARACTERS, XMLStreamConstants.CDATA, XMLStreamConstants.SPACE ->
                    return XmlPullParser.TEXT.also { event = it }
                XMLStreamConstants.END_DOCUMENT -> return XmlPullParser.END_DOCUMENT.also { event = it }
                else -> Unit // comments, processing instructions
            }
        }
        event = XmlPullParser.END_DOCUMENT
        return event
    }

    override fun getEventType(): Int = event
    override fun getName(): String? = if (event == XmlPullParser.START_TAG || event == XmlPullParser.END_TAG) reader?.localName else null
    override fun getText(): String? = if (event == XmlPullParser.TEXT) reader?.text else null
    override fun getNamespace(): String = reader?.namespaceURI.orEmpty()
    override fun getPrefix(): String? = reader?.prefix
    override fun getDepth(): Int = 0
    override fun getAttributeCount(): Int = if (event == XmlPullParser.START_TAG) reader?.attributeCount ?: 0 else -1
    override fun getAttributeName(index: Int): String? = reader?.getAttributeLocalName(index)
    override fun getAttributeValue(index: Int): String? = reader?.getAttributeValue(index)
    override fun getAttributeValue(namespace: String?, name: String?): String? = reader?.getAttributeValue(namespace, name)
    override fun getAttributeNamespace(index: Int): String = reader?.getAttributeNamespace(index).orEmpty()
    override fun getAttributePrefix(index: Int): String? = reader?.getAttributePrefix(index)
    override fun getAttributeType(index: Int): String = "CDATA"
    override fun isAttributeDefault(index: Int): Boolean = false
    override fun isEmptyElementTag(): Boolean = false
    override fun isWhitespace(): Boolean = getText()?.isBlank() == true
    override fun getInputEncoding(): String? = null
    override fun getPositionDescription(): String = "line ${getLineNumber()}"
    override fun getLineNumber(): Int = reader?.location?.lineNumber ?: -1
    override fun getColumnNumber(): Int = reader?.location?.columnNumber ?: -1

    override fun defineEntityReplacementText(entityName: String?, replacementText: String?) = unsupported()
    override fun getNamespaceCount(depth: Int): Int = unsupported()
    override fun getNamespacePrefix(pos: Int): String = unsupported()
    override fun getNamespaceUri(pos: Int): String = unsupported()
    override fun getNamespace(prefix: String?): String = unsupported()
    override fun getTextCharacters(holderForStartAndLength: IntArray?): CharArray = unsupported()
    override fun nextToken(): Int = next()
    override fun require(type: Int, namespace: String?, name: String?) = unsupported()
    override fun nextText(): String = unsupported()
    override fun nextTag(): Int = unsupported()

    private fun unsupported(): Nothing = throw UnsupportedOperationException("not used by Dlna.kt")
}
