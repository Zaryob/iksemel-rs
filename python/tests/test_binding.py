import gc
import pickle
import threading
import unittest
import iksemel as iks

class BindingTests(unittest.TestCase):
    def test_dom_and_lifetime(self):
        root = iks.parseString("<root a='one'><first>old</first><second/></root>")
        child = root.getTag('first')
        del root
        gc.collect()
        self.assertEqual(child.parent().name(), 'root')
        child.setData('new & 世')
        self.assertEqual(child.firstChild().data(), 'new & 世')
        self.assertEqual(child.root().getTagData('first'), 'new & 世')
        root = child.root()
        self.assertEqual(root.attributes(), ['a'])
        root.setAttribute('a', None)
        self.assertEqual(root.attributes(), [])
        child.appendTag('middle').insertData('text')
        child.prependTag('before')
        self.assertEqual([n.name() for n in root.tags()], ['before', 'first', 'middle', 'second'])
        self.assertEqual(child.nextTag('second').previousTag('first').name(), 'first')
        child.hide()
        self.assertIsNone(child.parent())
        self.assertEqual(child.toString(), '<first>new &amp; &#x4e16;</first>')
    def test_copy_pickle_and_errors(self):
        root = iks.newDocument('root')
        source = iks.parseString('<other><child/></other>')
        inserted = root.insertNode(source)
        source._document.close()
        self.assertEqual(inserted.toString(), '<other><child/></other>')
        self.assertEqual(pickle.loads(pickle.dumps(root)).toString(), root.toString())
        with self.assertRaises(iks.NotData): root.data()
        with self.assertRaises(iks.ParseError): iks.parseString('<open>')
        root._document.close()
        with self.assertRaises(ReferenceError): inserted.name()
    def test_thread_confinement(self):
        root = iks.newDocument('r')
        errors = []
        def access():
            try: root.name()
            except RuntimeError: errors.append(True)
        t = threading.Thread(target=access); t.start(); t.join()
        self.assertEqual(errors, [True])
    def test_pretty_mixed_content(self):
        root = iks.parseString('<r>Hello <b>world</b>!</r>')
        self.assertIn('Hello <b>world</b>!', root.toPrettyString())

if __name__ == '__main__': unittest.main()
