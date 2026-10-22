import copy
from pathlib import Path
import tempfile
import unittest
import xml.etree.ElementTree as ET

from prepare_runtime import export_models


class PhysicsExportTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="bv-sim-physics-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.output = self.root / "output"

    def model(self, name, content):
        directory = self.source / name
        directory.mkdir(parents=True)
        (directory / "model.sdf").write_text(f'<sdf version="1.9"><model name="{name}">{content}</model></sdf>')

    def test_exports_dependency_closure_without_changing_flight_physics(self):
        self.model("x500_gimbal", '<include><uri>model://base</uri></include><joint name="gimbal" type="revolute"><parent>body</parent><child>camera</child></joint>')
        self.model("unused_scenery", '<link name="ball"/>')
        body = '''<link name="body"><pose>0 0 0.1 0 0 0</pose>
          <inertial><mass>2.5</mass><inertia><ixx>0.03</ixx></inertia></inertial>
          <collision name="body"><geometry><box><size>1 2 3</size></box></geometry></collision>
          <sensor name="imu_sensor" type="imu"><update_rate>250</update_rate></sensor>
          <sensor name="boundingbox_camera" type="boundingbox_camera"><pose>0 0 0 0 0 0</pose></sensor>
          <visual name="body"><geometry><mesh><uri>model://base/mesh.dae</uri></mesh></geometry></visual>
          </link><plugin filename="motor-model" name="motor"><motorConstant>0.001</motorConstant></plugin>'''
        self.model("base", body)
        before = ET.parse(self.source / "base/model.sdf")
        # Compare every remaining physical component, including all numeric text.
        expected = copy.deepcopy(before)
        expected.find(".//link").remove(expected.find(".//visual"))
        names = export_models(self.source, self.output)
        self.assertEqual(names, {"x500_gimbal", "base"})
        self.assertFalse((self.output / "unused_scenery").exists())
        actual = ET.parse(self.output / "base/model.sdf")
        self.assertFalse(actual.findall(".//visual"))
        self.assertFalse(actual.findall(".//mesh"))
        def structure(node):
            return (node.tag, node.attrib, (node.text or "").strip(), [structure(child) for child in node])
        self.assertEqual(structure(actual.getroot()), structure(expected.getroot()))

    def test_rejects_physical_mesh_instead_of_silently_removing_collisions(self):
        self.model("x500_gimbal", '<link name="body"><collision name="body"><geometry><mesh><uri>model://vehicle/body.stl</uri></mesh></geometry></collision></link>')
        with self.assertRaisesRegex(ValueError, "physical mesh"):
            export_models(self.source, self.output)

    def test_rejects_remote_resources_and_escaping_includes(self):
        for uri in ["https://fuel.gazebosim.org/model", "model://../other"]:
            with self.subTest(uri=uri):
                if (self.source / "x500_gimbal").exists():
                    (self.source / "x500_gimbal/model.sdf").unlink()
                    (self.source / "x500_gimbal").rmdir()
                self.model("x500_gimbal", f'<include><uri>{uri}</uri></include>')
                with self.assertRaises(ValueError):
                    export_models(self.source, self.output)


if __name__ == "__main__":
    unittest.main()
